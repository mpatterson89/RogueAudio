mod commands;
mod covers;
mod downloads;
mod error;
mod plex;
mod storage;
mod user_collections;

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
            plex_list_collections,
            plex_collection_books,
            plex_get_book_detail,
            plex_get_playback,
            plex_get_stream,
            // Progress + Continue elsewhere (Plex sync)
            progress_get,
            progress_get_merged,
            progress_report,
            progress_clear,
            progress_sync_get_enabled,
            progress_sync_set_enabled,
            progress_sync_enable_and_merge,
            // Offline downloads
            download_list,
            download_get,
            download_enqueue,
            download_cancel,
            download_remove,
            download_remove_all,
            download_storage_bytes,
            download_local_playback,
            // Cover art cache
            cover_get_local,
            cover_ensure,
            cover_ensure_many,
            cover_import,
            // User collections (local)
            user_collections_list,
            user_collections_create,
            user_collections_rename,
            user_collections_delete,
            user_collections_add_books,
            user_collections_remove_books,
            user_collections_get,
        ])
        .run(tauri::generate_context!())
        .expect("error while running RogueAudio");
}
