mod commands;
mod error;
mod plex;
mod storage;

use commands::*;
pub use plex::PlexAuthState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(PlexAuthState::default())
        .invoke_handler(tauri::generate_handler![
            // Plex auth & library
            plex_start_pin_auth,
            plex_poll_pin_auth,
            plex_logout,
            plex_auth_status,
            plex_dev_complete_auth,
            plex_list_servers,
            plex_list_libraries,
            plex_list_books,
            plex_get_stream,
            // Progress
            progress_get,
            progress_report,
            // Downloads (stub)
            download_list,
            download_enqueue,
        ])
        .run(tauri::generate_context!())
        .expect("error while running RogueAudio");
}
