use crate::error::{AppError, AppResult};
use crate::plex::{self, ProgressReport, ProgressSnapshot, ProgressSource};
use crate::storage::app_data_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn progress_dir() -> AppResult<PathBuf> {
    let dir = app_data_dir()?.join("progress");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn progress_file(rating_key: &str) -> AppResult<PathBuf> {
    let safe: String = rating_key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    Ok(progress_dir()?.join(format!("{safe}.json")))
}

fn sync_prefs_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("progress-sync.json"))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncPrefs {
    version: u32,
    /// ratingKey → continue-elsewhere enabled
    enabled_by_rating_key: HashMap<String, bool>,
}

fn load_sync_prefs() -> AppResult<SyncPrefs> {
    let path = sync_prefs_path()?;
    if !path.exists() {
        return Ok(SyncPrefs {
            version: 1,
            enabled_by_rating_key: HashMap::new(),
        });
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

fn save_sync_prefs(prefs: &SyncPrefs) -> AppResult<()> {
    let path = sync_prefs_path()?;
    let raw = serde_json::to_string_pretty(prefs)
        .map_err(|e| AppError::Message(e.to_string()))?;
    fs::write(path, raw)?;
    Ok(())
}

fn read_local(rating_key: &str) -> AppResult<Option<ProgressSnapshot>> {
    let path = progress_file(rating_key)?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let snap: ProgressSnapshot = serde_json::from_str(&raw)
        .map_err(|e| AppError::Message(format!("invalid progress file: {e}")))?;
    Ok(Some(snap))
}

fn write_local(snap: &ProgressSnapshot) -> AppResult<()> {
    let path = progress_file(&snap.rating_key)?;
    let raw = serde_json::to_string_pretty(snap)
        .map_err(|e| AppError::Message(e.to_string()))?;
    fs::write(path, raw)?;
    Ok(())
}

fn parse_rfc3339_secs(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.timestamp())
}

/// Merge local + Plex: prefer newer timestamp; fall back to larger offset if &gt;5s apart.
fn merge_snapshots(
    rating_key: &str,
    local: Option<ProgressSnapshot>,
    plex: Option<plex::timeline::PlexProgress>,
) -> ProgressSnapshot {
    let now = chrono::Utc::now().to_rfc3339();
    match (local, plex) {
        (None, None) => ProgressSnapshot {
            rating_key: rating_key.to_string(),
            position_ms: 0,
            duration_ms: None,
            updated_at: now,
            source: ProgressSource::Local,
            track_index: None,
        },
        (Some(l), None) => l,
        (None, Some(p)) => ProgressSnapshot {
            rating_key: rating_key.to_string(),
            position_ms: p.position_ms,
            duration_ms: p.duration_ms,
            updated_at: p
                .last_viewed_at
                .map(|t| {
                    chrono::DateTime::from_timestamp(t, 0)
                        .map(|d| d.to_rfc3339())
                        .unwrap_or_else(|| now.clone())
                })
                .unwrap_or(now),
            source: ProgressSource::Plex,
            track_index: None,
        },
        (Some(l), Some(p)) => {
            let local_ts = parse_rfc3339_secs(&l.updated_at).unwrap_or(0);
            let plex_ts = p.last_viewed_at.unwrap_or(0);
            let delta = l.position_ms.abs_diff(p.position_ms);

            // Prefer newer source when timestamps available
            if plex_ts > 0 && local_ts > 0 {
                if plex_ts > local_ts + 2 && p.position_ms > 5_000 {
                    return ProgressSnapshot {
                        rating_key: rating_key.to_string(),
                        position_ms: p.position_ms,
                        duration_ms: p.duration_ms.or(l.duration_ms),
                        updated_at: now,
                        source: ProgressSource::Merged,
                        track_index: l.track_index,
                    };
                }
                if local_ts >= plex_ts {
                    return ProgressSnapshot {
                        rating_key: l.rating_key,
                        position_ms: l.position_ms,
                        duration_ms: l.duration_ms.or(p.duration_ms),
                        updated_at: l.updated_at,
                        source: ProgressSource::Merged,
                        track_index: l.track_index,
                    };
                }
            }

            // No reliable timestamps: take max if meaningfully different
            if delta > 5_000 {
                if p.position_ms > l.position_ms {
                    ProgressSnapshot {
                        rating_key: rating_key.to_string(),
                        position_ms: p.position_ms,
                        duration_ms: p.duration_ms.or(l.duration_ms),
                        updated_at: now,
                        source: ProgressSource::Merged,
                        track_index: l.track_index,
                    }
                } else {
                    ProgressSnapshot {
                        rating_key: l.rating_key,
                        position_ms: l.position_ms,
                        duration_ms: l.duration_ms.or(p.duration_ms),
                        updated_at: l.updated_at,
                        source: ProgressSource::Merged,
                        track_index: l.track_index,
                    }
                }
            } else {
                ProgressSnapshot {
                    rating_key: l.rating_key,
                    position_ms: l.position_ms,
                    duration_ms: l.duration_ms.or(p.duration_ms),
                    updated_at: l.updated_at,
                    source: ProgressSource::Merged,
                    track_index: l.track_index,
                }
            }
        }
    }
}

#[tauri::command]
pub fn progress_get(rating_key: String) -> AppResult<Option<ProgressSnapshot>> {
    read_local(&rating_key)
}

/// Local progress merged with Plex viewOffset when available.
#[tauri::command]
pub async fn progress_get_merged(
    server_id: String,
    rating_key: String,
) -> AppResult<ProgressSnapshot> {
    let local = read_local(&rating_key)?;
    let plex = plex::timeline::fetch_view_offset(&server_id, &rating_key)
        .await
        .ok()
        .flatten();
    let merged = merge_snapshots(&rating_key, local, plex);
    // Persist merge so next local-only read is consistent
    if merged.position_ms > 0 {
        write_local(&merged)?;
    }
    Ok(merged)
}

#[tauri::command]
pub async fn progress_report(
    report: ProgressReport,
    server_id: Option<String>,
    sync_to_plex: Option<bool>,
) -> AppResult<ProgressSnapshot> {
    let sync = sync_to_plex.unwrap_or(false);
    let snap = ProgressSnapshot {
        rating_key: report.rating_key.clone(),
        position_ms: report.time_ms,
        duration_ms: report.duration_ms,
        updated_at: chrono::Utc::now().to_rfc3339(),
        source: if sync {
            ProgressSource::Merged
        } else {
            ProgressSource::Local
        },
        track_index: report.track_index,
    };

    write_local(&snap)?;

    if sync {
        if let Some(server_id) = server_id.as_deref().filter(|s| !s.is_empty()) {
            if let Err(e) = plex::timeline::post_timeline(
                server_id,
                &report.rating_key,
                &report.state,
                report.time_ms,
                report.duration_ms,
                report.speed,
            )
            .await
            {
                // Local already saved — log and continue
                eprintln!("plex timeline post failed: {e}");
            }
        }
    }

    Ok(snap)
}

/// Delete saved progress for a title (optional reset).
#[tauri::command]
pub fn progress_clear(rating_key: String) -> AppResult<()> {
    let path = progress_file(&rating_key)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[tauri::command]
pub fn progress_sync_get_enabled(rating_key: String) -> AppResult<bool> {
    let prefs = load_sync_prefs()?;
    Ok(*prefs
        .enabled_by_rating_key
        .get(&rating_key)
        .unwrap_or(&false))
}

#[tauri::command]
pub fn progress_sync_set_enabled(rating_key: String, enabled: bool) -> AppResult<bool> {
    let mut prefs = load_sync_prefs()?;
    if enabled {
        prefs.enabled_by_rating_key.insert(rating_key, true);
    } else {
        prefs.enabled_by_rating_key.remove(&rating_key);
    }
    prefs.version = 1;
    save_sync_prefs(&prefs)?;
    Ok(enabled)
}

/// Enable sync, merge local↔Plex, return merged snapshot (and push local if it wins).
#[tauri::command]
pub async fn progress_sync_enable_and_merge(
    server_id: String,
    rating_key: String,
) -> AppResult<ProgressSnapshot> {
    progress_sync_set_enabled(rating_key.clone(), true)?;
    let local = read_local(&rating_key)?;
    let plex = plex::timeline::fetch_view_offset(&server_id, &rating_key)
        .await
        .ok()
        .flatten();
    let merged = merge_snapshots(&rating_key, local.clone(), plex.clone());
    write_local(&merged)?;

    // If local was ahead (or equal), push so Plexamp matches this device
    let should_push = match (&local, &plex) {
        (Some(l), Some(p)) => l.position_ms + 5_000 >= p.position_ms,
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (None, None) => false,
    };
    if should_push && merged.position_ms > 0 {
        let _ = plex::timeline::post_timeline(
            &server_id,
            &rating_key,
            "paused",
            merged.position_ms,
            merged.duration_ms,
            1.0,
        )
        .await;
    }

    Ok(merged)
}
