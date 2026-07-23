use crate::downloads::{self, DownloadItem, DownloadManager, LocalPlayback};
use crate::error::AppResult;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn download_list() -> AppResult<Vec<DownloadItem>> {
    downloads::list_downloads()
}

#[tauri::command]
pub fn download_get(rating_key: String) -> AppResult<Option<DownloadItem>> {
    downloads::get_download(&rating_key)
}

/// Start (or no-op if already complete) an offline download for a book.
/// Returns immediately; progress is streamed via the `download-progress` event.
#[tauri::command]
pub async fn download_enqueue(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
    server_id: String,
    rating_key: String,
) -> AppResult<DownloadItem> {
    if !crate::plex::is_authenticated() {
        return Err(crate::error::AppError::NotAuthenticated);
    }

    // Already finished
    if let Some(item) = downloads::get_download(&rating_key)? {
        if item.status == "complete" {
            return Ok(item);
        }
        if item.status == "downloading" || item.status == "queued" {
            // Job may still be running
            if manager.is_active(&rating_key) {
                return Ok(item);
            }
        }
    }

    let Some(cancel) = manager.try_begin(&rating_key) else {
        if let Some(item) = downloads::get_download(&rating_key)? {
            return Ok(item);
        }
        return Ok(DownloadItem {
            rating_key,
            server_id,
            title: "Downloading…".into(),
            author: None,
            series: None,
            series_index: None,
            status: "downloading".into(),
            progress: 0.0,
            error: None,
            tracks_done: 0,
            track_count: 0,
            bytes_downloaded: 0,
            bytes_total: None,
            bytes_on_disk: 0,
            duration_ms: None,
            cover_path: None,
            downloaded_at: None,
            file_names: vec![],
        });
    };

    let mgr = manager.inner().clone();
    let app2 = app.clone();
    let sid = server_id.clone();
    let rk = rating_key.clone();

    // Detach long-running work so the IPC call returns immediately.
    tauri::async_runtime::spawn(async move {
        if let Err(e) = downloads::run_download(app2, mgr, cancel, sid, rk.clone()).await {
            eprintln!("download {rk} failed: {e}");
        }
    });

    // Optimistic UI state until the first download-progress event arrives
    Ok(DownloadItem {
        rating_key,
        server_id,
        title: "Downloading…".into(),
        author: None,
        series: None,
        series_index: None,
        status: "queued".into(),
        progress: 0.0,
        error: None,
        tracks_done: 0,
        track_count: 0,
        bytes_downloaded: 0,
        bytes_total: None,
        bytes_on_disk: 0,
        duration_ms: None,
        cover_path: None,
        downloaded_at: None,
        file_names: vec![],
    })
}

#[tauri::command]
pub fn download_cancel(
    manager: State<'_, Arc<DownloadManager>>,
    rating_key: String,
) -> AppResult<()> {
    manager.request_cancel(&rating_key);
    Ok(())
}

#[tauri::command]
pub fn download_remove(
    manager: State<'_, Arc<DownloadManager>>,
    rating_key: String,
) -> AppResult<()> {
    manager.request_cancel(&rating_key);
    downloads::remove_download(&rating_key)
}

/// Delete all offline audiobook folders. Returns how many book folders were removed.
#[tauri::command]
pub fn download_remove_all(manager: State<'_, Arc<DownloadManager>>) -> AppResult<u32> {
    // Best-effort: cancel any active jobs first by listing known keys
    if let Ok(items) = downloads::list_downloads() {
        for item in items {
            manager.request_cancel(&item.rating_key);
        }
    }
    downloads::remove_all_downloads()
}

/// Total disk bytes used by offline downloads.
#[tauri::command]
pub fn download_storage_bytes() -> AppResult<u64> {
    downloads::total_storage_bytes()
}

/// Local playback playlist when a complete download exists (absolute file paths).
#[tauri::command]
pub fn download_local_playback(rating_key: String) -> AppResult<Option<LocalPlayback>> {
    downloads::local_playback(&rating_key)
}
