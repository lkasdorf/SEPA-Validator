mod commands;
mod formatting;
mod model;
mod payments;
mod scanner;
mod schema;
mod validator;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_validation,
            commands::read_file,
            commands::write_text_file,
            commands::read_formatted,
            commands::read_payment_summary
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
