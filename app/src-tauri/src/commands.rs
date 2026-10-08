use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::model::{Message, Severity, Status, ValidationResult};
use crate::scanner;
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
        }],
    }
}

/// Read a file's text for the code viewer (lossy UTF-8).
#[tauri::command]
pub fn read_file(path: String) -> Result<String, String> {
    std::fs::read(&path)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .map_err(|e| e.to_string())
}

/// Write text to an absolute path chosen via the save dialog.
#[tauri::command]
pub fn write_text_file(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|e| e.to_string())
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
#[tauri::command]
pub fn read_payment_summary(path: String) -> Result<crate::payments::PaymentSummary, String> {
    crate::payments::extract_payment_summary(std::path::Path::new(&path))
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

fn is_zip(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

/// Extract every `.xsd` entry of a zip into `dest`, flattened to its basename
/// (so archive subfolders and any `../` are neutralized). Returns
/// (imported_count, skipped). On open/read failure the zip path is added to skipped.
fn extract_zip_xsds(zip_path: &Path, dest: &Path) -> (u32, Vec<String>) {
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
        let base = name
            .rsplit(|c| c == '/' || c == '\\')
            .next()
            .unwrap_or("")
            .to_string();
        if base.is_empty() || !base.to_lowercase().ends_with(".xsd") {
            continue;
        }
        match std::fs::File::create(dest.join(&base)) {
            Ok(mut out) => {
                if std::io::copy(&mut entry, &mut out).is_ok() {
                    imported += 1;
                } else {
                    skipped.push(format!("{}!{}", zip_path.display(), base));
                }
            }
            Err(_) => skipped.push(format!("{}!{}", zip_path.display(), base)),
        }
    }
    (imported, skipped)
}

fn copy_one(src: &Path, dest: &Path, imported: &mut u32, skipped: &mut Vec<String>) {
    match src.file_name() {
        Some(name) if std::fs::copy(src, dest.join(name)).is_ok() => *imported += 1,
        _ => skipped.push(src.display().to_string()),
    }
}

/// Copy `.xsd` files from the given paths (files or directories) into `dest`.
/// Pure (no AppHandle) for testability.
pub fn copy_xsds(paths: &[String], dest: &Path) -> ImportResult {
    let mut imported = 0u32;
    let mut skipped: Vec<String> = Vec::new();
    for p in paths {
        let path = Path::new(p);
        if path.is_dir() {
            match std::fs::read_dir(path) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let ep = entry.path();
                        if ep.is_file() && is_xsd(&ep) {
                            copy_one(&ep, dest, &mut imported, &mut skipped);
                        }
                    }
                }
                Err(_) => skipped.push(p.clone()),
            }
        } else if path.is_file() && is_zip(path) {
            let (imp, mut skp) = extract_zip_xsds(path, dest);
            imported += imp;
            skipped.append(&mut skp);
        } else if path.is_file() && is_xsd(path) {
            copy_one(path, dest, &mut imported, &mut skipped);
        } else {
            skipped.push(p.clone());
        }
    }
    ImportResult { imported, skipped }
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
    std::process::Command::new("explorer")
        .arg(&dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Open a URL in the default browser (Windows).
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(&url)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
