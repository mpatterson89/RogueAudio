mod commands;
mod downloads;
mod error;
mod plex;
mod storage;

use commands::*;
use downloads::DownloadManager;
use std::sync::Arc;
pub use plex::PlexAuthState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(PlexAuthState::default())
        .manage(Arc::new(DownloadManager::default()))
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
            plex_get_book_detail,
            plex_get_playback,
            plex_get_stream,
            // Progress
            progress_get,
            progress_report,
            progress_clear,
            // Offline downloads
            download_list,
            download_get,
            download_enqueue,
            download_cancel,
            download_remove,
            download_local_playback,
        ])
        .run(tauri::generate_context!())
        .expect("error while running RogueAudio");
}
