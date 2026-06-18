use std::path::PathBuf;

use serde::Serialize;
use tauri::ipc::Channel;

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

/// Expand inputs, then validate each file on a worker thread, streaming
/// results to the frontend in order via the channel.
#[tauri::command]
pub fn start_validation(paths: Vec<String>, on_event: Channel<ValidationEvent>) {
    let files: Vec<PathBuf> = scanner::expand_paths(paths.iter().map(PathBuf::from));
    let total = files.len();

    // libxml types are not Send: build the Validator inside the thread.
    std::thread::spawn(move || {
        let _ = on_event.send(ValidationEvent::Started { total });
        let mut validator = Validator::new();
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
