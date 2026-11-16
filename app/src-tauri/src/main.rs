// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(structure_core::session::Session::new())
        .invoke_handler(tauri::generate_handler![
            commands::analyze,
            commands::reclassify,
            commands::rules_stamp,
            commands::impact,
            commands::history,
            commands::cell_symbols,
            commands::diff
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("error while running tauri application: {e}");
        std::process::exit(1);
    }
}
