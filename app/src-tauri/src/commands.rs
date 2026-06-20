use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::model::ValidationResult;
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
    },
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
/// results to the frontend in order via the channel.
#[tauri::command]
pub fn start_validation(app: AppHandle, paths: Vec<String>, on_event: Channel<ValidationEvent>) {
    let files: Vec<PathBuf> = scanner::expand_paths(paths.iter().map(PathBuf::from));
    let total = files.len();
    let dir = schema_dir(&app).unwrap_or_default();

    // libxml types are not Send: build the Validator inside the thread.
    std::thread::spawn(move || {
        let _ = on_event.send(ValidationEvent::Started { total });
        let mut validator = Validator::new(dir);
        for (index, file) in files.iter().enumerate() {
            let result = validator.validate_file(file);
            let _ = on_event.send(ValidationEvent::Result { index, result });
        }
        let _ = on_event.send(ValidationEvent::Finished { total });
    });
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
