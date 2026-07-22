use crate::error::{AppError, AppResult};
use crate::plex::{ProgressReport, ProgressSnapshot, ProgressSource};
use crate::storage::app_data_dir;
use std::fs;
use std::path::PathBuf;

fn progress_dir() -> AppResult<PathBuf> {
    let dir = app_data_dir()?.join("progress");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn progress_file(rating_key: &str) -> AppResult<PathBuf> {
    // Sanitize key for filesystem
    let safe: String = rating_key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    Ok(progress_dir()?.join(format!("{safe}.json")))
}

#[tauri::command]
pub fn progress_get(rating_key: String) -> AppResult<Option<ProgressSnapshot>> {
    let path = progress_file(&rating_key)?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let snap: ProgressSnapshot = serde_json::from_str(&raw)
        .map_err(|e| AppError::Message(format!("invalid progress file: {e}")))?;
    Ok(Some(snap))
}

#[tauri::command]
pub fn progress_report(report: ProgressReport) -> AppResult<ProgressSnapshot> {
    // Local write first — reliability over network.
    let snap = ProgressSnapshot {
        rating_key: report.rating_key.clone(),
        position_ms: report.time_ms,
        duration_ms: report.duration_ms,
        updated_at: chrono::Utc::now().to_rfc3339(),
        source: ProgressSource::Local,
    };

    let path = progress_file(&report.rating_key)?;
    let raw = serde_json::to_string_pretty(&snap)
        .map_err(|e| AppError::Message(e.to_string()))?;
    fs::write(path, raw)?;

    // TODO: POST to Plex :/timeline/ and merge source → Plex / Merged
    let _state = report.state;
    let _speed = report.speed;

    Ok(snap)
}
