use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItem {
    pub rating_key: String,
    pub title: String,
    pub status: String,
    pub progress: f32,
}

#[tauri::command]
pub fn download_list() -> AppResult<Vec<DownloadItem>> {
    Ok(vec![])
}

#[tauri::command]
pub fn download_enqueue(_rating_key: String) -> AppResult<()> {
    Err(AppError::NotImplemented(
        "offline downloads land after player + progress foundation",
    ))
}
