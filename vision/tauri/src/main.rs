// Tauri main — keeps the desktop wrapper minimal. The window is configured
// declaratively in tauri.conf.json and points at the bundled web app
// (or http://127.0.0.1:7777 in dev).

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    tauri::Builder::default()
        .setup(|_app| {
            // Local-only by policy: never bind anything to 0.0.0.0 from this process.
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running mneme vision");
}
