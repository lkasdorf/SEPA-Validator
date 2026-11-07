use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::model::{Message, Severity, Status, ValidationResult};
use crate::scanner;
use crate::statement_gen::{Generated, StatementPreview};
use crate::validator::Validator;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum ValidationEvent {
    Started {
        total: usize,
    },
    Result {
        index: usize,
        result: ValidationResult,
    },
    Finished {
        total: usize,
        /// True if the run stopped early (cancelled or superseded by a newer run).
        cancelled: bool,
    },
}

/// Generation counter for validation runs. Starting a run or cancelling bumps
/// it, so a worker whose id is no longer current stops before its next file.
#[derive(Default, Clone)]
pub struct RunState(Arc<AtomicU64>);

impl RunState {
    /// Start a new run (implicitly cancelling any running one); returns its id.
    pub fn begin(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn cancel(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self, id: u64) -> bool {
        self.0.load(Ordering::SeqCst) != id
    }
}

/// The per-user directory that holds imported XSD schema files (created if missing).
pub fn schema_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("schemas");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Expand inputs, then validate each file on a worker thread, streaming
/// results to the frontend in order via the channel. Starting a run cancels
/// the previous one.
#[tauri::command]
pub fn start_validation(
    app: AppHandle,
    runs: State<'_, RunState>,
    paths: Vec<String>,
    on_event: Channel<ValidationEvent>,
) {
    let runs = runs.inner().clone();
    let id = runs.begin();
    let dir = schema_dir(&app).unwrap_or_default();

    // libxml types are not Send: build the Validator inside the thread.
    // The folder walk runs there too, so dropping a big folder can't block the UI thread.
    std::thread::spawn(move || {
        let files: Vec<PathBuf> = scanner::expand_paths(paths.iter().map(PathBuf::from));
        let mut validator = Validator::new(dir);
        run_batch(
            &files,
            |f| validator.validate_file(f),
            || runs.is_cancelled(id),
            |ev| on_event.send(ev).is_ok(),
        );
    });
}

/// Stop the running validation after the file it is currently validating.
#[tauri::command]
pub fn cancel_validation(runs: State<'_, RunState>) {
    runs.cancel();
}

/// Validate `files` in order, emitting Started, one Result per file, then Finished.
/// A panic while validating one file becomes an Error result for that file, so
/// the run always continues and always finishes (the UI waits for Finished).
/// Stops early when `is_cancelled` turns true or `send` reports the frontend gone.
fn run_batch(
    files: &[PathBuf],
    mut validate: impl FnMut(&Path) -> ValidationResult,
    is_cancelled: impl Fn() -> bool,
    mut send: impl FnMut(ValidationEvent) -> bool,
) {
    let total = files.len();
    if !send(ValidationEvent::Started { total }) {
        return;
    }
    for (index, file) in files.iter().enumerate() {
        if is_cancelled() {
            send(ValidationEvent::Finished {
                total,
                cancelled: true,
            });
            return;
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| validate(file)))
            .unwrap_or_else(|payload| crashed_result(file, payload.as_ref()));
        if !send(ValidationEvent::Result { index, result }) {
            return;
        }
    }
    send(ValidationEvent::Finished {
        total,
        cancelled: false,
    });
}

fn crashed_result(file: &Path, payload: &(dyn std::any::Any + Send)) -> ValidationResult {
    let reason = payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown error".into());
    ValidationResult {
        file: file
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string(),
        path: file.display().to_string(),
        namespace: String::new(),
        schema: String::new(),
        status: Status::Error,
        errors: 1,
        warnings: 0,
        messages: vec![Message {
            severity: Severity::Error,
            text: format!("Internal error while validating this file: {reason}"),
            line: None,
            column: None,
            hint: None,
        }],
    }
}

/// Write an export (chosen via the save dialog) to disk; see `check_export_path`.
#[tauri::command]
pub fn write_text_file(path: String, contents: String) -> Result<(), String> {
    let path = check_export_path(&path, &["txt", "csv"])?;
    std::fs::write(path, contents).map_err(|e| e.to_string())
}

/// Save the pretty-printed XML of `source` (as shown in the viewer) to `target`.
#[tauri::command]
pub fn save_formatted(source: String, target: String) -> Result<(), String> {
    write_formatted_copy(Path::new(&source), &target)
}

/// Formats `source` in the backend (the webview only names the files) and writes
/// it to `target`: an `.xml` path per `check_export_path`, never the original
/// itself. Formatting only re-indents between tags; values stay byte-identical.
fn write_formatted_copy(source: &Path, target: &str) -> Result<(), String> {
    let target = check_export_path(target, &["xml"])?;
    let same_file = std::fs::canonicalize(source)
        .ok()
        .is_some_and(|s| std::fs::canonicalize(&target).ok() == Some(s));
    if same_file {
        return Err("Save the formatted file as a copy; the original stays untouched.".into());
    }
    let formatted = crate::formatting::format_xml(source).map_err(|e| match e.line {
        Some(line) => format!("Cannot format: {} (line {line})", e.message),
        None => format!("Cannot format: {}", e.message),
    })?;
    std::fs::write(target, formatted).map_err(|e| e.to_string())
}

/// Exports may only create files with one of `extensions` in an existing folder
/// on a local drive: nothing the webview could write and later run, no UNC or
/// device paths, no alternate data streams. (Windows-only app.)
fn check_export_path(path: &str, extensions: &[&str]) -> Result<PathBuf, String> {
    let refuse = || format!("Refusing to write to {path}");
    let p = PathBuf::from(path);
    if !on_local_drive(&p) {
        return Err(refuse());
    }
    let name = p.file_name().and_then(|n| n.to_str()).ok_or_else(refuse)?;
    let export_ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| extensions.iter().any(|x| e.eq_ignore_ascii_case(x)));
    if !export_ext || name.contains(':') || !p.parent().is_some_and(Path::is_dir) {
        return Err(refuse());
    }
    Ok(p)
}

/// An absolute path on a local drive letter (`C:\…`), not UNC or a device path.
fn on_local_drive(p: &Path) -> bool {
    use std::path::{Component, Prefix};
    p.is_absolute()
        && matches!(
            p.components().next(),
            Some(Component::Prefix(pre)) if matches!(pre.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        )
}

/// Statements are written next to the source, which the export guard only
/// allows on a local drive; say so up front instead of a bare refusal.
fn check_statement_source(source: &Path) -> Result<(), String> {
    if on_local_drive(source) {
        Ok(())
    } else {
        Err("Statements are saved next to the payment file, which must be on a local drive. Copy the file to a local folder first.".into())
    }
}

/// What *Generate statement* would write for the payment file `source`.
#[tauri::command]
pub fn preview_statements(source: String) -> Result<StatementPreview, String> {
    check_statement_source(Path::new(&source))?;
    let xml = read_utf8(Path::new(&source))?;
    Ok(crate::statement_gen::preview(
        &xml,
        &crate::plausibility::today(),
    ))
}

/// Write synthetic camt.053 statements for `source` next to it and return their
/// paths. `opening_balance` is the first day's balance as typed (`10000.00`).
#[tauri::command]
pub fn generate_statements(source: String, opening_balance: String) -> Result<Vec<String>, String> {
    write_statements(
        Path::new(&source),
        &opening_balance,
        &crate::plausibility::today(),
        &crate::statement_gen::now_utc(),
    )
}

fn read_utf8(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|_| "The file is not UTF-8.".to_string())
}

fn write_statements(
    source: &Path,
    opening: &str,
    today: &str,
    now: &str,
) -> Result<Vec<String>, String> {
    check_statement_source(source)?;
    let opening = crate::statement_gen::parse_opening(opening)?;
    let xml = read_utf8(source)?;
    let name = source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let generated = crate::statement_gen::generate(&xml, name, opening, today, now)
        .map_err(|r| r.join("\n"))?;
    let targets = statement_targets(source, &generated)?;
    let mut written: Vec<String> = Vec::new();
    for (g, target) in generated.iter().zip(&targets) {
        match write_new(target, &g.xml) {
            Ok(p) => written.push(p.display().to_string()),
            Err(e) if written.is_empty() => return Err(e),
            Err(e) => return Err(format!("{e}\nAlready written: {}", written.join(", "))),
        }
    }
    Ok(written)
}

/// The preferred target of each statement (before any `_1` collision suffix):
/// `<stem>_camt053[_<account tag>]_YYYYMMDD.xml` next to `source`.
fn statement_targets(source: &Path, generated: &[Generated]) -> Result<Vec<PathBuf>, String> {
    let dir = source.parent().ok_or("The source file has no folder.")?;
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("The source file has no name.")?;
    let several = generated.iter().any(|g| g.iban != generated[0].iban);
    Ok(generated
        .iter()
        .map(|g| {
            let account = if several {
                format!("_{}", g.tag)
            } else {
                String::new()
            };
            dir.join(format!(
                "{stem}_camt053{account}_{}.xml",
                g.date.replace('-', "")
            ))
        })
        .collect())
}

/// Create `preferred` (or `<stem>_1.xml`, `_2`, … if it exists) through the
/// export guard. `create_new` makes sure no existing file, the source included,
/// is ever overwritten.
fn write_new(preferred: &Path, contents: &str) -> Result<PathBuf, String> {
    use std::io::Write;
    let stem = preferred
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    for i in 0..1000 {
        let candidate = if i == 0 {
            preferred.to_path_buf()
        } else {
            preferred.with_file_name(format!("{stem}_{i}.xml"))
        };
        let target = check_export_path(&candidate.display().to_string(), &["xml"])?;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut f) => {
                f.write_all(contents.as_bytes())
                    .map_err(|e| format!("Could not write {}: {e}", target.display()))?;
                return Ok(target);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("Could not write {}: {e}", target.display())),
        }
    }
    Err(format!(
        "No free file name next to {}.",
        preferred.display()
    ))
}

/// Return the pretty-printed XML for the viewer. Falls back to raw bytes if the
/// file isn't well-formed (so the user still sees the content).
#[tauri::command]
pub fn read_formatted(path: String) -> Result<String, String> {
    match crate::formatting::format_xml(std::path::Path::new(&path)) {
        Ok(s) => Ok(s),
        Err(_) => std::fs::read(&path)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .map_err(|e| e.to_string()),
    }
}

/// Extract the per-file SEPA payment summary (PmtInf stats + Ustrd list).
/// Statement entries get the GVC's name when the DK mapping table is imported.
#[tauri::command]
pub fn read_payment_summary(
    app: AppHandle,
    path: String,
) -> Result<crate::payments::PaymentSummary, String> {
    let mut summary = crate::payments::extract_payment_summary(std::path::Path::new(&path))?;
    if summary.transactions.iter().any(|t| t.gvc.is_some()) {
        if let Some(table) = schema_dir(&app)
            .ok()
            .and_then(|d| crate::gvc::GvcTable::load(&d))
        {
            add_gvc_names(&mut summary.transactions, &table);
        }
    }
    Ok(summary)
}

/// Fills `gvc_text` from the mapping table.
fn add_gvc_names(rows: &mut [crate::payments::RemittanceEntry], table: &crate::gvc::GvcTable) {
    for t in rows {
        if let Some(gvc) = &t.gvc {
            t.gvc_text = table
                .name(gvc, t.iso_tx_code.as_deref(), t.credit_debit.as_deref())
                .map(str::to_string);
        }
    }
}

/// The imported GVC mapping table, if any: where it came from and how many codes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GvcInfo {
    pub source: String,
    pub codes: u32,
}

#[tauri::command]
pub fn gvc_status(app: AppHandle) -> Result<Option<GvcInfo>, String> {
    let dir = schema_dir(&app)?;
    Ok(crate::gvc::GvcTable::load(&dir).map(|t| GvcInfo {
        source: t.source.clone(),
        codes: t.codes() as u32,
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaInfo {
    pub namespace: String,
    pub filename: String,
    pub present: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: u32,
    pub skipped: Vec<String>,
    /// Number of GVCs if a DK mapping table (.xlsx) was imported.
    pub gvc_codes: Option<u32>,
}

/// Return the known schemas with a present/absent flag for the schema dir.
#[tauri::command]
pub fn schema_status(app: AppHandle) -> Result<Vec<SchemaInfo>, String> {
    let dir = schema_dir(&app)?;
    Ok(crate::schema::known_schemas()
        .iter()
        .map(|(ns, filename)| SchemaInfo {
            namespace: (*ns).to_string(),
            filename: (*filename).to_string(),
            present: dir.join(filename).exists(),
        })
        .collect())
}

fn is_xsd(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("xsd"))
        .unwrap_or(false)
}

fn is_xlsx(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("xlsx"))
}

/// Converts a DK GVC mapping workbook and stores it in `dest`; the number of
/// GVCs on success.
fn import_gvc_table(table: Result<crate::gvc::GvcTable, String>, dest: &Path) -> Option<u32> {
    let table = table.ok()?;
    table.save(dest).ok()?;
    Some(table.codes() as u32)
}

fn is_zip(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

/// Extract every `.xsd` entry of a zip into `dest`, flattened to its basename
/// (so archive subfolders and any `../` are neutralized), and convert a DK GVC
/// mapping workbook (`.xlsx`) if the zip has one. Returns (imported_count,
/// skipped). On open/read failure the zip path is added to skipped.
fn extract_zip_xsds(zip_path: &Path, dest: &Path, gvc: &mut Option<u32>) -> (u32, Vec<String>) {
    let mut imported = 0u32;
    let mut skipped: Vec<String> = Vec::new();
    let file = match std::fs::File::open(zip_path) {
        Ok(f) => f,
        Err(_) => {
            skipped.push(zip_path.display().to_string());
            return (imported, skipped);
        }
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => {
            skipped.push(zip_path.display().to_string());
            return (imported, skipped);
        }
    };
    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let base = name.rsplit(['/', '\\']).next().unwrap_or("").to_string();
        if base.to_lowercase().ends_with(".xlsx") {
            if let Some(bytes) = crate::gvc::read_capped(&mut entry) {
                let found = import_gvc_table(crate::gvc::parse_workbook(&bytes, &base), dest);
                *gvc = found.or(*gvc);
            }
            continue;
        }
        if base.is_empty() || !base.to_lowercase().ends_with(".xsd") {
            continue;
        }
        let ok = is_safe_xsd_name(&base)
            && entry.size() <= MAX_XSD_BYTES
            && write_capped(&mut entry, &dest.join(&base)).is_ok();
        if ok {
            imported += 1;
        } else {
            skipped.push(format!("{}!{}", zip_path.display(), name));
        }
    }
    (imported, skipped)
}

fn copy_one(src: &Path, dest: &Path, imported: &mut u32, skipped: &mut Vec<String>) {
    let copied = src.file_name().is_some_and(|name| {
        std::fs::File::open(src)
            .and_then(|mut f| write_capped(&mut f, &dest.join(name)))
            .is_ok()
    });
    if copied {
        *imported += 1;
    } else {
        skipped.push(src.display().to_string());
    }
}

/// Largest schema file we import (the ISO/SIX XSDs are well under 1 MB).
const MAX_XSD_BYTES: u64 = 10 * 1024 * 1024;

/// A plain file name for an imported schema. Rejects anything Windows would
/// resolve elsewhere, such as drive-relative `C:x.xsd` or a stream `a:b.xsd`.
fn is_safe_xsd_name(name: &str) -> bool {
    name.len() <= 200
        && !name.starts_with('.')
        && name.to_ascii_lowercase().ends_with(".xsd")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Copy at most `MAX_XSD_BYTES` into `target` through a temp file, so an
/// oversized or failed copy never leaves a truncated schema behind.
fn write_capped(src: &mut impl std::io::Read, target: &Path) -> std::io::Result<()> {
    let tmp = target.with_extension("xsd.part");
    let result = (|| {
        let mut out = std::fs::File::create(&tmp)?;
        let n = std::io::copy(
            &mut std::io::Read::take(src.by_ref(), MAX_XSD_BYTES + 1),
            &mut out,
        )?;
        if n > MAX_XSD_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "schema file too large",
            ));
        }
        drop(out);
        std::fs::rename(&tmp, target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Copy `.xsd` files from the given paths (files or directories) into `dest`.
/// Pure (no AppHandle) for testability.
pub fn copy_xsds(paths: &[String], dest: &Path) -> ImportResult {
    let mut imported = 0u32;
    let mut skipped: Vec<String> = Vec::new();
    let mut gvc_codes: Option<u32> = None;
    for p in paths {
        let path = Path::new(p);
        if path.is_dir() {
            match std::fs::read_dir(path) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let ep = entry.path();
                        if ep.is_file() && is_xsd(&ep) {
                            copy_one(&ep, dest, &mut imported, &mut skipped);
                        } else if ep.is_file() && is_xlsx(&ep) {
                            // Other workbooks in the folder are simply not GVC tables.
                            let found =
                                import_gvc_table(crate::gvc::parse_workbook_file(&ep), dest);
                            gvc_codes = found.or(gvc_codes);
                        }
                    }
                }
                Err(_) => skipped.push(p.clone()),
            }
        } else if path.is_file() && is_zip(path) {
            let (imp, mut skp) = extract_zip_xsds(path, dest, &mut gvc_codes);
            imported += imp;
            skipped.append(&mut skp);
        } else if path.is_file() && is_xsd(path) {
            copy_one(path, dest, &mut imported, &mut skipped);
        } else if path.is_file() && is_xlsx(path) {
            match import_gvc_table(crate::gvc::parse_workbook_file(path), dest) {
                Some(n) => gvc_codes = Some(n),
                None => skipped.push(p.clone()),
            }
        } else {
            skipped.push(p.clone());
        }
    }
    ImportResult {
        imported,
        skipped,
        gvc_codes,
    }
}

/// Copy selected `.xsd` files/folders into the schema dir.
#[tauri::command]
pub fn import_schemas(app: AppHandle, paths: Vec<String>) -> Result<ImportResult, String> {
    let dir = schema_dir(&app)?;
    Ok(copy_xsds(&paths, &dir))
}

/// Open the schema dir in the OS file explorer (Windows).
#[tauri::command]
pub fn open_schema_dir(app: AppHandle) -> Result<(), String> {
    let dir = schema_dir(&app)?;
    explorer().arg(&dir).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// True when this exe is the portable copy rather than an NSIS installation.
/// The updater can only update an installation (it runs the NSIS installer),
/// so the update dialog offers the new portable file instead.
#[tauri::command]
pub fn is_portable() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(is_portable_dir))
        .unwrap_or(false)
}

/// The NSIS installer puts `uninstall.exe` next to the app; a portable copy has none.
fn is_portable_dir(dir: &Path) -> bool {
    !dir.join("uninstall.exe").is_file()
}

/// Open a URL in the default browser (Windows). Only allow-listed https links.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let url = check_url(&url)?;
    explorer()
        .arg(url.as_str())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Hosts the app links to (repo, schema download pages, online IBAN check).
const ALLOWED_URL_HOSTS: &[&str] = &[
    "github.com",
    "www.ebics.de",
    "www.six-group.com",
    "www.ibancalculator.com",
];

/// Only `https://` links to `ALLOWED_URL_HOSTS`, with no credentials or custom
/// port. Everything else (local or UNC paths, `file:`, protocol handlers such as
/// `ms-msdt:`) is refused, because explorer would happily launch it.
fn check_url(url: &str) -> Result<tauri::Url, String> {
    let refuse = || format!("Refusing to open {url}");
    let parsed = tauri::Url::parse(url).map_err(|_| refuse())?;
    let allowed = parsed.scheme() == "https"
        && parsed
            .host_str()
            .is_some_and(|h| ALLOWED_URL_HOSTS.contains(&h))
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.port().is_none();
    if allowed {
        Ok(parsed)
    } else {
        Err(refuse())
    }
}

/// `explorer.exe` by absolute path, so a same-named program next to the app or
/// in the working directory can't be picked up instead.
fn explorer() -> std::process::Command {
    let exe = std::env::var_os("WINDIR")
        .map(|w| PathBuf::from(w).join("explorer.exe"))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("explorer"));
    std::process::Command::new(exe)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_gvc_workbook_is_imported_from_xlsx_zip_and_folder() {
        let bytes = crate::gvc::tests::workbook(&[(
            "166",
            "SEPA Credit Transfer (Einzelbuchung-Haben)",
            "C",
            "PMNT",
            "RCDT",
            "ESCT",
        )]);
        let root = std::env::temp_dir().join("sepa_gvc_import");
        let _ = std::fs::remove_dir_all(&root);
        let (src, dest) = (root.join("src"), root.join("dest"));
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dest).unwrap();

        // Plain .xlsx
        let xlsx = src.join("Anhang1.xlsx");
        std::fs::write(&xlsx, &bytes).unwrap();
        let r = copy_xsds(&[xlsx.display().to_string()], &dest);
        assert_eq!((r.imported, r.gvc_codes), (0, Some(1)));
        let t = crate::gvc::GvcTable::load(&dest).unwrap();
        assert_eq!(t.source, "Anhang1.xlsx");
        std::fs::remove_file(dest.join(crate::gvc::FILE_NAME)).unwrap();

        // Inside the ZIP from ebics.de
        let zip_path = src.join("Mapping.zip");
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("Mapping/Anhang1.xlsx", opts).unwrap();
            std::io::Write::write_all(&mut zw, &bytes).unwrap();
            zw.finish().unwrap();
        }
        let r = copy_xsds(&[zip_path.display().to_string()], &dest);
        assert_eq!(r.gvc_codes, Some(1));
        std::fs::remove_file(dest.join(crate::gvc::FILE_NAME)).unwrap();

        // A folder with other workbooks next to it: those are ignored, not skipped.
        std::fs::write(src.join("other.xlsx"), b"not a workbook").unwrap();
        let r = copy_xsds(&[src.display().to_string()], &dest);
        assert_eq!(r.gvc_codes, Some(1));
        assert!(r.skipped.is_empty(), "{:?}", r.skipped);

        // A single .xlsx that is no GVC table is reported as skipped.
        let r = copy_xsds(&[src.join("other.xlsx").display().to_string()], &dest);
        assert_eq!((r.gvc_codes, r.skipped.len()), (None, 1));
    }

    #[test]
    fn gvc_names_are_added_to_statement_rows() {
        let bytes = crate::gvc::tests::workbook(&[
            ("201", "Zahlungsauftrag", "D", "PMNT", "ICDT", "XBCT"),
            (
                "201",
                "Rückgabe Zahlungseingang",
                "D",
                "PMNT",
                "RCDT",
                "XRTN",
            ),
        ]);
        let table = crate::gvc::parse_workbook(&bytes, "t.xlsx").unwrap();
        let mut rows = vec![
            crate::payments::RemittanceEntry {
                gvc: Some("201".into()),
                iso_tx_code: Some("PMNT-RCDT-XRTN".into()),
                credit_debit: Some("DBIT".into()),
                ..Default::default()
            },
            crate::payments::RemittanceEntry::default(),
        ];
        add_gvc_names(&mut rows, &table);
        assert_eq!(
            rows[0].gvc_text.as_deref(),
            Some("Rückgabe Zahlungseingang")
        );
        assert_eq!(rows[1].gvc_text, None);
    }

    use super::*;
    use std::io::Write;

    fn ok_result(p: &Path) -> ValidationResult {
        ValidationResult::from_messages(
            p.display().to_string(),
            p.display().to_string(),
            String::new(),
            String::new(),
            Vec::new(),
        )
    }

    #[test]
    fn a_panicking_file_does_not_stop_the_run() {
        let files: Vec<PathBuf> = ["a.xml", "boom.xml", "c.xml"]
            .iter()
            .map(PathBuf::from)
            .collect();
        let mut events = Vec::new();
        run_batch(
            &files,
            |p| {
                if p.ends_with("boom.xml") {
                    panic!("simulated crash");
                }
                ok_result(p)
            },
            || false,
            |ev| {
                events.push(ev);
                true
            },
        );

        let results: Vec<&ValidationResult> = events
            .iter()
            .filter_map(|e| match e {
                ValidationEvent::Result { result, .. } => Some(result),
                _ => None,
            })
            .collect();
        assert_eq!(results.len(), 3);
        assert_eq!(results[1].status, crate::model::Status::Error);
        assert_eq!(results[2].status, crate::model::Status::Ok);
        assert!(matches!(
            events.last(),
            Some(ValidationEvent::Finished {
                total: 3,
                cancelled: false
            })
        ));
    }

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn a_cancelled_run_stops_before_the_next_file_and_says_so() {
        let files = paths(&["a.xml", "b.xml", "c.xml"]);
        let validated = std::cell::Cell::new(0);
        let mut events = Vec::new();
        run_batch(
            &files,
            |p| {
                validated.set(validated.get() + 1);
                ok_result(p)
            },
            || validated.get() >= 1,
            |ev| {
                events.push(ev);
                true
            },
        );
        assert_eq!(validated.get(), 1);
        assert!(matches!(
            events.last(),
            Some(ValidationEvent::Finished {
                total: 3,
                cancelled: true
            })
        ));
    }

    #[test]
    fn a_run_stops_when_its_events_can_no_longer_be_delivered() {
        let files = paths(&["a.xml", "b.xml"]);
        let validated = std::cell::Cell::new(0);
        run_batch(
            &files,
            |p| {
                validated.set(validated.get() + 1);
                ok_result(p)
            },
            || false,
            |_| false,
        );
        assert_eq!(validated.get(), 0);
    }

    #[test]
    fn starting_a_new_run_cancels_the_previous_one() {
        let runs = RunState::default();
        let a = runs.begin();
        assert!(!runs.is_cancelled(a));
        let b = runs.begin();
        assert!(runs.is_cancelled(a));
        assert!(!runs.is_cancelled(b));
    }

    #[test]
    fn cancel_stops_the_current_run() {
        let runs = RunState::default();
        let a = runs.begin();
        runs.cancel();
        assert!(runs.is_cancelled(a));
    }

    fn fresh_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_file(p: &Path, content: &str) {
        let mut f = std::fs::File::create(p).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn copies_xsd_file_and_skips_non_xsd() {
        let src = fresh_dir("sepa_imp_src1");
        let dest = fresh_dir("sepa_imp_dest1");
        write_file(&src.join("pain.001.001.03.xsd"), "<xsd/>");
        write_file(&src.join("notes.txt"), "x");
        let xsd = src.join("pain.001.001.03.xsd").display().to_string();
        let txt = src.join("notes.txt").display().to_string();
        let r = copy_xsds(&[xsd, txt.clone()], &dest);
        assert_eq!(r.imported, 1);
        assert_eq!(r.skipped, vec![txt]);
        assert!(dest.join("pain.001.001.03.xsd").exists());
    }

    #[test]
    fn extracts_xsd_from_zip_and_ignores_non_xsd() {
        use std::io::Write as _;
        let dest = fresh_dir("sepa_imp_destzip");
        let zip_path = std::env::temp_dir().join("sepa_imp_test.zip");
        let _ = std::fs::remove_file(&zip_path);
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            // A nested .xsd (must be flattened to its basename) and a non-.xsd.
            zw.start_file("schemas/pain.001.001.03.xsd", opts).unwrap();
            zw.write_all(b"<xsd/>").unwrap();
            zw.start_file("readme.txt", opts).unwrap();
            zw.write_all(b"hi").unwrap();
            zw.finish().unwrap();
        }
        let r = copy_xsds(&[zip_path.display().to_string()], &dest);
        assert_eq!(r.imported, 1);
        assert!(dest.join("pain.001.001.03.xsd").exists());
        assert!(!dest.join("readme.txt").exists());
    }

    #[test]
    fn copies_all_xsd_from_directory_case_insensitive() {
        let src = fresh_dir("sepa_imp_src2");
        let dest = fresh_dir("sepa_imp_dest2");
        write_file(&src.join("a.xsd"), "<a/>");
        write_file(&src.join("b.XSD"), "<b/>");
        write_file(&src.join("c.txt"), "c");
        let r = copy_xsds(&[src.display().to_string()], &dest);
        assert_eq!(r.imported, 2);
        assert!(dest.join("a.xsd").exists());
        assert!(dest.join("b.XSD").exists());
        assert!(!dest.join("c.txt").exists());
    }

    #[test]
    fn open_url_allows_https_to_known_hosts() {
        for u in [
            "https://github.com/lkasdorf/SEPA-Validator/issues/new",
            "https://www.ebics.de/de/datenformate",
            "https://www.six-group.com/en/products-services/banking-services/payment-standardization/standards/iso-20022.html",
            "https://www.ibancalculator.com/validate/DE89370400440532013000",
        ] {
            assert!(check_url(u).is_ok(), "{u}");
        }
    }

    #[test]
    fn open_url_rejects_everything_else() {
        for u in [
            "http://github.com/",
            "https://evil.example/",
            "https://github.com.evil.example/",
            "https://ibancalculator.com/validate/DE89370400440532013000",
            "https://user@github.com/",
            "https://github.com:8443/",
            "file:///C:/Windows/System32/calc.exe",
            r"C:\Windows\System32\calc.exe",
            r"\\attacker\share\x.exe",
            "ms-msdt:/id PCWDiagnostic",
            "search-ms:query=x",
            "javascript:alert(1)",
            "",
        ] {
            assert!(check_url(u).is_err(), "{u}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn export_path_allows_txt_and_csv_in_existing_folders() {
        let dir = fresh_dir("sepa_export_ok");
        for name in ["report.csv", "report.TXT"] {
            let p = dir.join(name).display().to_string();
            assert!(check_export_path(&p, &["txt", "csv"]).is_ok(), "{p}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn export_path_rejects_other_targets() {
        let dir = fresh_dir("sepa_export_bad");
        let d = dir.display().to_string();
        for p in [
            format!(r"{d}\evil.bat"),
            format!(r"{d}\evil.ps1"),
            format!(r"{d}\noext"),
            format!(r"{d}\a.bat:b.txt"),
            format!(r"{d}\missing\report.csv"),
            "report.csv".to_string(),
            r"\\attacker\share\report.csv".to_string(),
            r"\\.\C:\report.csv".to_string(),
        ] {
            assert!(check_export_path(&p, &["txt", "csv"]).is_err(), "{p}");
        }
    }

    fn zip_with(path: &Path, entries: &[(&str, Vec<u8>)]) {
        use std::io::Write as _;
        let _ = std::fs::remove_file(path);
        let f = std::fs::File::create(path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            zw.start_file(*name, opts).unwrap();
            zw.write_all(data).unwrap();
        }
        zw.finish().unwrap();
    }

    #[test]
    fn zip_entries_with_drive_relative_names_are_skipped() {
        let dest = fresh_dir("sepa_imp_drive_rel");
        let zip_path = std::env::temp_dir().join("sepa_imp_drive_rel.zip");
        zip_with(
            &zip_path,
            &[
                ("C:evil.xsd", b"<x/>".to_vec()),
                ("sub/C:evil2.xsd", b"<x/>".to_vec()),
                ("ok.xsd", b"<x/>".to_vec()),
            ],
        );
        let r = copy_xsds(&[zip_path.display().to_string()], &dest);
        assert_eq!(r.imported, 1, "skipped: {:?}", r.skipped);
        assert_eq!(r.skipped.len(), 2, "skipped: {:?}", r.skipped);
        assert!(!Path::new("evil.xsd").exists() && !Path::new("evil2.xsd").exists());
        assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 1);
    }

    #[test]
    fn oversized_zip_entries_are_skipped_without_leftovers() {
        let dest = fresh_dir("sepa_imp_oversize");
        let zip_path = std::env::temp_dir().join("sepa_imp_oversize.zip");
        let huge = vec![b' '; (MAX_XSD_BYTES + 1) as usize];
        zip_with(
            &zip_path,
            &[("huge.xsd", huge), ("ok.xsd", b"<x/>".to_vec())],
        );
        let r = copy_xsds(&[zip_path.display().to_string()], &dest);
        assert_eq!(r.imported, 1, "skipped: {:?}", r.skipped);
        assert!(!dest.join("huge.xsd").exists());
        assert_eq!(
            std::fs::read_dir(&dest).unwrap().count(),
            1,
            "no partial files"
        );
    }

    #[test]
    fn save_formatted_writes_an_indented_copy() {
        let dir = fresh_dir("sepa_save_fmt_ok");
        let src = dir.join("in.xml");
        write_file(&src, r#"<?xml version="1.0"?><a><b> x </b><c></c></a>"#);
        let target = dir.join("in_formatted.xml");
        write_formatted_copy(&src, &target.display().to_string()).unwrap();
        let out = std::fs::read_to_string(&target).unwrap();
        assert_eq!(out, crate::formatting::format_xml(&src).unwrap());
        assert!(out.lines().count() > 2, "got:\n{out}");
    }

    #[test]
    fn save_formatted_never_overwrites_the_original() {
        let dir = fresh_dir("sepa_save_fmt_same");
        let src = dir.join("in.xml");
        let original = r#"<?xml version="1.0"?><a><b>1</b></a>"#;
        write_file(&src, original);
        assert!(write_formatted_copy(&src, &src.display().to_string()).is_err());
        assert_eq!(std::fs::read_to_string(&src).unwrap(), original);
    }

    #[cfg(windows)]
    #[test]
    fn save_formatted_only_writes_xml_files() {
        let dir = fresh_dir("sepa_save_fmt_ext");
        let src = dir.join("in.xml");
        write_file(&src, "<a/>");
        let target = dir.join("evil.bat");
        assert!(write_formatted_copy(&src, &target.display().to_string()).is_err());
        assert!(!target.exists());
    }

    #[test]
    fn save_formatted_refuses_malformed_files() {
        let dir = fresh_dir("sepa_save_fmt_bad");
        let src = dir.join("in.xml");
        write_file(&src, "<a><b>");
        let target = dir.join("out.xml");
        assert!(write_formatted_copy(&src, &target.display().to_string()).is_err());
        assert!(!target.exists());
    }
    #[test]
    fn a_folder_with_the_nsis_uninstaller_is_an_installation() {
        let dir = fresh_dir("sepa_installed_layout");
        write_file(&dir.join("uninstall.exe"), "");
        assert!(!is_portable_dir(&dir));
    }

    #[test]
    fn a_folder_without_an_uninstaller_is_a_portable_copy() {
        let dir = fresh_dir("sepa_portable_layout");
        assert!(is_portable_dir(&dir));
    }

    const TODAY: &str = "2026-11-07";
    const NOW: &str = "2026-11-07T09:30:00";
    const SRC_NAME: &str = "20261107_TEST_PAIN001.xml";

    /// A synthetic pain.001 (test IBANs, invented names) with one block per date.
    fn synthetic_pain001(ccy: &str, dates: &[&str]) -> String {
        let blocks: String = dates
            .iter()
            .map(|d| {
                format!(
                    "<PmtInf><PmtInfId>P-{d}</PmtInfId><ReqdExctnDt>{d}</ReqdExctnDt>\
                     <Dbtr><Nm>Example Owner Ltd</Nm></Dbtr>\
                     <DbtrAcct><Id><IBAN>DE09000000000000000001</IBAN></Id></DbtrAcct>\
                     <CdtTrfTxInf><PmtId><EndToEndId>E-{d}</EndToEndId></PmtId>\
                     <Amt><InstdAmt Ccy=\"{ccy}\">10.00</InstdAmt></Amt><Cdtr><Nm>Supplier One</Nm></Cdtr>\
                     <CdtrAcct><Id><IBAN>DE52000000000000000003</IBAN></Id></CdtrAcct></CdtTrfTxInf></PmtInf>"
                )
            })
            .collect();
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:pain.001.001.03\">\
             <CstmrCdtTrfInitn><GrpHdr><MsgId>M</MsgId></GrpHdr>{blocks}</CstmrCdtTrfInitn></Document>\n"
        )
    }

    fn generated(iban: &str, date: &str) -> Generated {
        Generated {
            iban: iban.into(),
            tag: iban[iban.len() - 4..].into(),
            date: date.into(),
            xml: String::new(),
        }
    }

    #[test]
    fn network_source_is_refused_with_a_reason() {
        let unc = r"\\server\share\20261107_TEST_PAIN001.xml";
        let err = write_statements(Path::new(unc), "1.00", TODAY, NOW).unwrap_err();
        assert!(err.contains("local drive"), "{err}");
        let err = preview_statements(unc.into()).unwrap_err();
        assert!(err.contains("local drive"), "{err}");
    }

    #[test]
    fn targets_use_the_account_tag() {
        let src = fresh_dir("sepa_stmt_targets_tag").join(SRC_NAME);
        let mut a = generated("DE57000000000000010001", "2026-12-01");
        let mut b = generated("DE08000000000000020001", "2026-12-01");
        a.tag = "10001".into();
        b.tag = "20001".into();
        assert_eq!(
            names(&statement_targets(&src, &[a, b]).unwrap()),
            [
                "20261107_TEST_PAIN001_camt053_10001_20261201.xml",
                "20261107_TEST_PAIN001_camt053_20001_20261201.xml"
            ]
        );
    }

    fn names(paths: &[PathBuf]) -> Vec<String> {
        paths
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn targets_single_account() {
        let dir = fresh_dir("sepa_stmt_targets_single");
        let src = dir.join(SRC_NAME);
        let t = statement_targets(
            &src,
            &[
                generated("DE09000000000000000001", "2026-12-01"),
                generated("DE09000000000000000001", "2026-12-03"),
            ],
        )
        .unwrap();
        assert_eq!(
            names(&t),
            [
                "20261107_TEST_PAIN001_camt053_20261201.xml",
                "20261107_TEST_PAIN001_camt053_20261203.xml"
            ]
        );
        assert!(t.iter().all(|p| p.parent() == Some(dir.as_path())));
    }

    #[test]
    fn targets_multi_account() {
        let src = fresh_dir("sepa_stmt_targets_multi").join(SRC_NAME);
        let t = statement_targets(
            &src,
            &[
                generated("DE79000000000000000002", "2026-12-01"),
                generated("DE09000000000000000001", "2026-12-01"),
            ],
        )
        .unwrap();
        assert_eq!(
            names(&t),
            [
                "20261107_TEST_PAIN001_camt053_0002_20261201.xml",
                "20261107_TEST_PAIN001_camt053_0001_20261201.xml"
            ]
        );
    }

    #[test]
    fn second_run_does_not_overwrite() {
        let dir = fresh_dir("sepa_stmt_second_run");
        let src = dir.join(SRC_NAME);
        let source_xml = synthetic_pain001("EUR", &["2026-12-01"]);
        write_file(&src, &source_xml);
        let first = write_statements(&src, "10000.00", TODAY, NOW).unwrap();
        assert_eq!(first.len(), 1);
        assert!(
            first[0].ends_with("20261107_TEST_PAIN001_camt053_20261201.xml"),
            "{first:?}"
        );
        let before = std::fs::read(&first[0]).unwrap();
        let second = write_statements(&src, "500.00", TODAY, NOW).unwrap();
        assert!(
            second[0].ends_with("20261107_TEST_PAIN001_camt053_20261201_1.xml"),
            "{second:?}"
        );
        assert_eq!(std::fs::read(&first[0]).unwrap(), before);
        assert_eq!(std::fs::read_to_string(&src).unwrap(), source_xml);
        let written = std::fs::read_to_string(&second[0]).unwrap();
        assert!(written.contains("camt.053.001.08") && written.contains("500.00"));
    }

    #[test]
    fn refuses_bad_opening() {
        let dir = fresh_dir("sepa_stmt_bad_opening");
        let src = dir.join(SRC_NAME);
        write_file(&src, &synthetic_pain001("EUR", &["2026-12-01"]));
        let err = write_statements(&src, "10.000,00", TODAY, NOW).unwrap_err();
        assert!(err.contains("10000.00"), "{err}");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn refused_file_writes_nothing() {
        let dir = fresh_dir("sepa_stmt_refused");
        let src = dir.join(SRC_NAME);
        write_file(&src, &synthetic_pain001("USD", &["2026-12-01"]));
        let err = write_statements(&src, "10000.00", TODAY, NOW).unwrap_err();
        assert!(err.contains("USD"), "{err}");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn non_utf8_source_is_refused() {
        let dir = fresh_dir("sepa_stmt_latin1");
        let src = dir.join(SRC_NAME);
        std::fs::write(&src, b"<Document>\xe4</Document>").unwrap();
        assert_eq!(
            write_statements(&src, "1.00", TODAY, NOW).unwrap_err(),
            "The file is not UTF-8."
        );
    }
}
