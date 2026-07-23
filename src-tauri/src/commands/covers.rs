use crate::covers::{self, CoverEnsureRequest, CoverEnsureResult};
use crate::error::AppResult;

/// Absolute path if a local cover already exists (cache or download folder).
#[tauri::command]
pub fn cover_get_local(server_id: String, rating_key: String) -> AppResult<Option<String>> {
    Ok(covers::find_local(&server_id, &rating_key)?
        .map(|p| p.to_string_lossy().to_string()))
}

/// Download cover if missing; return absolute path.
#[tauri::command]
pub async fn cover_ensure(
    server_id: String,
    rating_key: String,
    remote_url: String,
) -> AppResult<String> {
    covers::ensure(&server_id, &rating_key, &remote_url).await
}

/// Batch ensure; per-item errors are returned in the payload (does not fail the whole batch).
#[tauri::command]
pub async fn cover_ensure_many(requests: Vec<CoverEnsureRequest>) -> Vec<CoverEnsureResult> {
    covers::ensure_many(requests).await
}

/// Copy an existing image file into the shared cover cache.
#[tauri::command]
pub fn cover_import(server_id: String, rating_key: String, source_path: String) -> AppResult<String> {
    covers::import_file(&server_id, &rating_key, std::path::Path::new(&source_path))
}
