use crate::downloads::{
    self, DownloadItem, DownloadManager, LocalPlayback, QueueState,
};
use crate::error::AppResult;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn download_list(manager: State<'_, Arc<DownloadManager>>) -> AppResult<Vec<DownloadItem>> {
    let mut items = downloads::list_downloads()?;
    let order = manager.order();
    let index: std::collections::HashMap<&str, u32> = order
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i as u32))
        .collect();
    for item in items.iter_mut() {
        item.queue_index = index.get(item.rating_key.as_str()).copied();
    }
    Ok(items)
}

#[tauri::command]
pub fn download_get(rating_key: String) -> AppResult<Option<DownloadItem>> {
    downloads::get_download(&rating_key)
}

/// Add a book to the offline download queue (starts when the worker is free).
#[tauri::command]
pub fn download_enqueue(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
    server_id: String,
    rating_key: String,
) -> AppResult<DownloadItem> {
    if !crate::plex::is_authenticated() {
        return Err(crate::error::AppError::NotAuthenticated);
    }
    downloads::enqueue(&app, manager.inner(), server_id, rating_key)
}

/// Cancel one queued/active item (keeps complete downloads untouched if not active).
#[tauri::command]
pub fn download_cancel(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
    rating_key: String,
) -> AppResult<()> {
    downloads::cancel_item(&app, manager.inner(), &rating_key)
}

/// Pause the entire download queue (partials kept for resume).
#[tauri::command]
pub fn download_pause_queue(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
) -> AppResult<QueueState> {
    downloads::pause_queue(&app, manager.inner())
}

/// Resume the queue and continue interrupted / paused jobs.
#[tauri::command]
pub fn download_resume_queue(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
) -> AppResult<QueueState> {
    if !crate::plex::is_authenticated() {
        return Err(crate::error::AppError::NotAuthenticated);
    }
    downloads::resume_queue(&app, manager.inner())
}

/// Queue totals + pause flag (also emitted as `download-queue` events).
#[tauri::command]
pub fn download_queue_state(
    manager: State<'_, Arc<DownloadManager>>,
) -> AppResult<QueueState> {
    Ok(downloads::queue_state(manager.inner()))
}

/// Cold-start restore: heal interrupted jobs; auto-resume if the queue was active.
#[tauri::command]
pub fn download_restore(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
) -> AppResult<QueueState> {
    downloads::restore_queue(&app, manager.inner())
}

#[tauri::command]
pub fn download_remove(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
    rating_key: String,
) -> AppResult<()> {
    downloads::remove_item(&app, manager.inner(), &rating_key)
}

/// Delete all offline audiobook folders. Returns how many book folders were removed.
#[tauri::command]
pub fn download_remove_all(
    app: AppHandle,
    manager: State<'_, Arc<DownloadManager>>,
) -> AppResult<u32> {
    downloads::remove_all_items(&app, manager.inner())
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

/// True when offline files are ready for HTML5 playback (MP3 or already web-safe).
#[tauri::command]
pub fn download_playable_ready(rating_key: String) -> AppResult<bool> {
    Ok(downloads::playable_ready(&rating_key))
}

/// Ensure WebKit-safe MP3 sidecars exist (ffmpeg). Can take a while for long books.
#[tauri::command]
pub async fn download_ensure_playable(rating_key: String) -> AppResult<downloads::DownloadItem> {
    let rk = rating_key.clone();
    let item = tauri::async_runtime::spawn_blocking(move || downloads::ensure_playable(&rk))
        .await
        .map_err(|e| crate::error::AppError::Message(format!("ensure_playable task: {e}")))??;
    Ok(item)
}
