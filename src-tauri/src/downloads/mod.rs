//! Offline audiobook downloads.
//!
//! Stores progressive MP3 tracks (same transcoder path as online playback) under
//! `~/.local/share/rogue-audio/downloads/{ratingKey}/` plus a manifest.json.

use crate::error::{AppError, AppResult};
use crate::plex::{self, BookChapter, BookDetail, PlaybackInfo, PlaybackTrack};
use crate::storage::app_data_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const MANIFEST_VERSION: u32 = 1;
const EVENT_PROGRESS: &str = "download-progress";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Complete,
    Error,
    Cancelled,
}

impl DownloadStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Complete => "complete",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTrackMeta {
    pub index: u32,
    pub rating_key: String,
    pub title: String,
    pub duration_ms: Option<u64>,
    pub file_name: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadManifest {
    pub version: u32,
    pub rating_key: String,
    pub server_id: String,
    pub title: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub year: Option<i32>,
    pub duration_ms: Option<u64>,
    pub library_key: Option<String>,
    pub studio: Option<String>,
    pub chapters: Vec<BookChapter>,
    pub tracks: Vec<DownloadTrackMeta>,
    pub cover_file: Option<String>,
    pub status: DownloadStatus,
    /// Whole-book progress 0.0 ..= 1.0 (not per-chapter).
    pub progress: f32,
    pub error: Option<String>,
    pub tracks_done: u32,
    pub track_count: u32,
    pub bytes_downloaded: u64,
    /// Best-effort whole-book size estimate (grows if the stream is larger).
    #[serde(default)]
    pub bytes_total: Option<u64>,
    pub downloaded_at: Option<String>,
    pub updated_at: String,
}

/// UI-facing download snapshot (camelCase over the wire).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItem {
    pub rating_key: String,
    pub server_id: String,
    pub title: String,
    pub author: Option<String>,
    pub status: String,
    /// Whole-book progress 0.0 ..= 1.0
    pub progress: f32,
    pub error: Option<String>,
    pub tracks_done: u32,
    pub track_count: u32,
    pub bytes_downloaded: u64,
    pub bytes_total: Option<u64>,
    pub duration_ms: Option<u64>,
    /// Absolute path to cover when present (frontend converts via convertFileSrc).
    pub cover_path: Option<String>,
    pub downloaded_at: Option<String>,
}

impl From<&DownloadManifest> for DownloadItem {
    fn from(m: &DownloadManifest) -> Self {
        let cover_path = m.cover_file.as_ref().and_then(|name| {
            book_dir(&m.rating_key)
                .ok()
                .map(|d| d.join(name).to_string_lossy().to_string())
        });
        Self {
            rating_key: m.rating_key.clone(),
            server_id: m.server_id.clone(),
            title: m.title.clone(),
            author: m.author.clone(),
            status: m.status.as_str().to_string(),
            progress: m.progress,
            error: m.error.clone(),
            tracks_done: m.tracks_done,
            track_count: m.track_count,
            bytes_downloaded: m.bytes_downloaded,
            bytes_total: m.bytes_total,
            duration_ms: m.duration_ms,
            cover_path,
            downloaded_at: m.downloaded_at.clone(),
        }
    }
}

/// Matches `maxAudioBitrate=320` on the universal transcoder.
const TRANSCODE_BITRATE_BPS: f64 = 320_000.0;

/// Estimated on-disk size for a progressive MP3 of the given duration.
fn estimate_mp3_bytes(duration_ms: Option<u64>) -> Option<u64> {
    let ms = duration_ms.filter(|&d| d > 0)?;
    let secs = ms as f64 / 1000.0;
    // 320 kbps + small container overhead
    Some((secs * TRANSCODE_BITRATE_BPS / 8.0 * 1.02).round() as u64)
}

/// Whole-book progress from completed parts + the in-flight part.
///
/// `current_expected` is Content-Length or a duration-based estimate for the
/// active part. If the stream grows past that, the total expands so progress
/// never freezes near 95% for multi-hour single-file books.
fn whole_book_progress(
    completed_bytes: u64,
    current_bytes: u64,
    current_expected: Option<u64>,
    future_part_estimates: &[u64],
) -> (f32, u64) {
    let current_total = match current_expected {
        Some(expected) if expected > current_bytes => expected,
        // Underestimated or unknown length: keep ~15% headroom so the bar can move
        Some(_) | None if current_bytes > 0 => {
            let headroom = (current_bytes as f64 * 0.15).round() as u64;
            current_bytes + headroom.max(256 * 1024)
        }
        _ => current_expected.unwrap_or(1).max(1),
    };
    let future: u64 = future_part_estimates.iter().copied().sum();
    let total = completed_bytes
        .saturating_add(current_total)
        .saturating_add(future)
        .max(1);
    let done = completed_bytes.saturating_add(current_bytes);
    let progress = (done as f64 / total as f64).clamp(0.0, 0.999) as f32;
    (progress, total)
}

/// In-flight cancel flags, keyed by rating key.
#[derive(Default)]
pub struct DownloadManager {
    cancel: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl DownloadManager {
    pub fn request_cancel(&self, rating_key: &str) {
        if let Ok(map) = self.cancel.lock() {
            if let Some(flag) = map.get(rating_key) {
                flag.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Claim a download slot. Returns `None` if this title is already downloading.
    pub fn try_begin(&self, rating_key: &str) -> Option<Arc<AtomicBool>> {
        let mut map = self.cancel.lock().ok()?;
        if map.contains_key(rating_key) {
            return None;
        }
        let flag = Arc::new(AtomicBool::new(false));
        map.insert(rating_key.to_string(), Arc::clone(&flag));
        Some(flag)
    }

    fn unregister(&self, rating_key: &str) {
        if let Ok(mut map) = self.cancel.lock() {
            map.remove(rating_key);
        }
    }

    pub fn is_active(&self, rating_key: &str) -> bool {
        self.cancel
            .lock()
            .map(|m| m.contains_key(rating_key))
            .unwrap_or(false)
    }
}

pub fn downloads_root() -> AppResult<PathBuf> {
    let dir = app_data_dir()?.join("downloads");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn safe_key(rating_key: &str) -> String {
    rating_key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

pub fn book_dir(rating_key: &str) -> AppResult<PathBuf> {
    let dir = downloads_root()?.join(safe_key(rating_key));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn manifest_path(rating_key: &str) -> AppResult<PathBuf> {
    Ok(book_dir(rating_key)?.join("manifest.json"))
}

pub fn read_manifest(rating_key: &str) -> AppResult<Option<DownloadManifest>> {
    let path = manifest_path(rating_key)?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)?;
    let m: DownloadManifest = serde_json::from_str(&raw)
        .map_err(|e| AppError::Message(format!("invalid download manifest: {e}")))?;
    Ok(Some(m))
}

fn write_manifest(m: &DownloadManifest) -> AppResult<()> {
    let path = manifest_path(&m.rating_key)?;
    let raw = serde_json::to_string_pretty(m)
        .map_err(|e| AppError::Message(e.to_string()))?;
    fs::write(path, raw)?;
    Ok(())
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn emit_item(app: &AppHandle, item: &DownloadItem) {
    let _ = app.emit(EVENT_PROGRESS, item);
}

pub fn list_downloads() -> AppResult<Vec<DownloadItem>> {
    let root = downloads_root()?;
    let mut out = Vec::new();
    if !root.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let manifest = entry.path().join("manifest.json");
        if !manifest.exists() {
            continue;
        }
        let raw = match fs::read_to_string(&manifest) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let m: DownloadManifest = match serde_json::from_str(&raw) {
            Ok(m) => m,
            Err(_) => continue,
        };
        out.push(DownloadItem::from(&m));
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(out)
}

pub fn get_download(rating_key: &str) -> AppResult<Option<DownloadItem>> {
    Ok(read_manifest(rating_key)?.as_ref().map(DownloadItem::from))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPlayback {
    pub playback: PlaybackInfo,
    pub chapters: Vec<BookChapter>,
    pub title: String,
    pub author: Option<String>,
}

/// Build PlaybackInfo pointing at local MP3 files (absolute paths in `url`).
pub fn local_playback(rating_key: &str) -> AppResult<Option<LocalPlayback>> {
    let Some(m) = read_manifest(rating_key)? else {
        return Ok(None);
    };
    if m.status != DownloadStatus::Complete {
        return Ok(None);
    }
    let dir = book_dir(rating_key)?;
    let mut tracks = Vec::new();
    for t in &m.tracks {
        let path = dir.join(&t.file_name);
        if !path.exists() {
            return Err(AppError::Message(format!(
                "downloaded track missing: {}",
                t.file_name
            )));
        }
        tracks.push(PlaybackTrack {
            rating_key: t.rating_key.clone(),
            title: t.title.clone(),
            index: t.index,
            duration_ms: t.duration_ms,
            url: path.to_string_lossy().to_string(),
            container: Some("mp3".into()),
        });
    }
    if tracks.is_empty() {
        return Ok(None);
    }
    Ok(Some(LocalPlayback {
        playback: PlaybackInfo {
            book_rating_key: m.rating_key,
            tracks,
            total_duration_ms: m.duration_ms,
        },
        chapters: m.chapters,
        title: m.title,
        author: m.author,
    }))
}

pub fn remove_download(rating_key: &str) -> AppResult<()> {
    let dir = downloads_root()?.join(safe_key(rating_key));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

/// HTTP client with a long timeout for multi-hour audiobook pulls.
fn download_http() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(format!(
            "RogueAudio/{} (download)",
            env!("CARGO_PKG_VERSION")
        ))
        .timeout(Duration::from_secs(60 * 60 * 6)) // 6h per request
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::Message(format!("download client: {e}")))
}

/// Download a URL to `dest`. `on_progress(downloaded, content_length)` is called
/// as chunks arrive (`content_length` from headers when the server sends it).
async fn download_url_to_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> AppResult<u64> {
    let tmp = dest.with_extension("partial");
    if tmp.exists() {
        let _ = fs::remove_file(&tmp);
    }

    let res = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Message(format!("download request failed: {e}")))?;

    if !res.status().is_success() {
        return Err(AppError::Message(format!(
            "download HTTP {}: {}",
            res.status(),
            dest.display()
        )));
    }

    let content_length = res.content_length().filter(|&n| n > 0);
    // Announce size as soon as headers are known (progress bar can scale correctly)
    on_progress(0, content_length);

    let mut file = fs::File::create(&tmp)?;
    let mut total = 0u64;
    let mut stream = res;
    loop {
        if cancel.load(Ordering::SeqCst) {
            drop(file);
            let _ = fs::remove_file(&tmp);
            return Err(AppError::Message("download cancelled".into()));
        }
        match stream.chunk().await {
            Ok(Some(chunk)) => {
                file.write_all(&chunk)?;
                total += chunk.len() as u64;
                on_progress(total, content_length);
            }
            Ok(None) => break,
            Err(e) => {
                drop(file);
                let _ = fs::remove_file(&tmp);
                return Err(AppError::Message(format!("download stream error: {e}")));
            }
        }
    }
    file.flush()?;
    drop(file);

    if total == 0 {
        let _ = fs::remove_file(&tmp);
        return Err(AppError::Message("download produced empty file".into()));
    }

    if dest.exists() {
        let _ = fs::remove_file(dest);
    }
    fs::rename(&tmp, dest)?;
    Ok(total)
}

async fn try_download_cover(
    client: &reqwest::Client,
    detail: &BookDetail,
    dir: &Path,
    cancel: &AtomicBool,
) -> Option<String> {
    let url = detail.thumb.as_ref().or(detail.art.as_ref())?;
    if cancel.load(Ordering::SeqCst) {
        return None;
    }
    let dest = dir.join("cover.jpg");
    match download_url_to_file(client, url, &dest, cancel, |_, _| {}).await {
        Ok(_) => Some("cover.jpg".into()),
        Err(_) => None,
    }
}

/// Run the full download job. Emits `download-progress` events.
///
/// `cancel` must come from [`DownloadManager::try_begin`] so the slot is claimed
/// before the async task is spawned.
pub async fn run_download(
    app: AppHandle,
    manager: Arc<DownloadManager>,
    cancel: Arc<AtomicBool>,
    server_id: String,
    rating_key: String,
) -> AppResult<DownloadItem> {
    // Already complete — no-op
    if let Some(m) = read_manifest(&rating_key)? {
        if m.status == DownloadStatus::Complete {
            manager.unregister(&rating_key);
            let item = DownloadItem::from(&m);
            emit_item(&app, &item);
            return Ok(item);
        }
    }

    let dir = book_dir(&rating_key)?;

    let mut manifest = DownloadManifest {
        version: MANIFEST_VERSION,
        rating_key: rating_key.clone(),
        server_id: server_id.clone(),
        title: "Downloading…".into(),
        author: None,
        summary: None,
        year: None,
        duration_ms: None,
        library_key: None,
        studio: None,
        chapters: vec![],
        tracks: vec![],
        cover_file: None,
        status: DownloadStatus::Downloading,
        progress: 0.0,
        error: None,
        tracks_done: 0,
        track_count: 0,
        bytes_downloaded: 0,
        bytes_total: None,
        downloaded_at: None,
        updated_at: now_rfc3339(),
    };
    write_manifest(&manifest)?;
    emit_item(&app, &DownloadItem::from(&manifest));

    let result = async {
        let detail = plex::get_book_detail(&server_id, &rating_key).await?;
        // Full book playlist: every audio part under the album/book (not a single chapter).
        let playback = plex::get_playback(&server_id, &rating_key).await?;

        if playback.tracks.is_empty() {
            return Err(AppError::Message("no playable tracks to download".into()));
        }

        manifest.title = detail.title.clone();
        manifest.author = detail.author.clone();
        manifest.summary = detail.summary.clone();
        manifest.year = detail.year;
        manifest.duration_ms = detail.duration_ms.or(playback.total_duration_ms);
        manifest.library_key = detail.library_key.clone();
        manifest.studio = detail.studio.clone();
        manifest.chapters = detail.chapters.clone();
        manifest.track_count = playback.tracks.len() as u32;
        // Initial whole-book size guess from total duration (320 kbps MP3)
        let part_estimates: Vec<u64> = playback
            .tracks
            .iter()
            .map(|t| estimate_mp3_bytes(t.duration_ms).unwrap_or(0))
            .collect();
        let book_estimate: u64 = part_estimates.iter().sum();
        let book_estimate = if book_estimate > 0 {
            book_estimate
        } else {
            estimate_mp3_bytes(manifest.duration_ms).unwrap_or(0)
        };
        manifest.bytes_total = (book_estimate > 0).then_some(book_estimate);
        manifest.updated_at = now_rfc3339();
        write_manifest(&manifest)?;
        emit_item(&app, &DownloadItem::from(&manifest));

        let client = download_http()?;

        // Cover is best-effort (does not affect book progress %)
        if let Some(cover) = try_download_cover(&client, &detail, &dir, &cancel).await {
            manifest.cover_file = Some(cover);
            write_manifest(&manifest)?;
        }

        let n_parts = playback.tracks.len();
        let mut track_metas: Vec<DownloadTrackMeta> = Vec::with_capacity(n_parts);
        let mut last_emit_bytes = 0u64;

        for (i, track) in playback.tracks.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::Message("download cancelled".into()));
            }

            let file_name = format!("track_{i:03}.mp3");
            let dest = dir.join(&file_name);
            let track_index = i as u32;
            let completed_bytes: u64 = track_metas.iter().map(|t| t.bytes).sum();
            let future_estimates: Vec<u64> = playback.tracks[i + 1..]
                .iter()
                .map(|t| estimate_mp3_bytes(t.duration_ms).unwrap_or(0))
                .collect();
            let duration_est = estimate_mp3_bytes(track.duration_ms);

            let bytes = download_url_to_file(
                &client,
                &track.url,
                &dest,
                &cancel,
                |n, content_length| {
                    // Prefer real Content-Length; else duration→bitrate estimate for this part
                    let expected = content_length.or(duration_est);
                    let (progress, total) =
                        whole_book_progress(completed_bytes, n, expected, &future_estimates);
                    manifest.progress = progress;
                    manifest.bytes_downloaded = completed_bytes.saturating_add(n);
                    manifest.bytes_total = Some(total);
                    manifest.tracks_done = i as u32; // current part not finished yet
                    manifest.updated_at = now_rfc3339();

                    // Throttle UI/disk: first chunk, every ~256 KiB, or end of growth spurts
                    let delta = manifest
                        .bytes_downloaded
                        .saturating_sub(last_emit_bytes);
                    if n == 0 || delta >= 256 * 1024 {
                        last_emit_bytes = manifest.bytes_downloaded;
                        let _ = write_manifest(&manifest);
                        emit_item(&app, &DownloadItem::from(&manifest));
                    }
                },
            )
            .await?;

            track_metas.push(DownloadTrackMeta {
                index: track_index,
                rating_key: track.rating_key.clone(),
                title: track.title.clone(),
                duration_ms: track.duration_ms,
                file_name,
                bytes,
            });

            let done_bytes: u64 = track_metas.iter().map(|t| t.bytes).sum();
            let remaining_est: u64 = future_estimates.iter().sum();
            let total_after = done_bytes.saturating_add(remaining_est).max(done_bytes);
            let progress_after = if remaining_est == 0 {
                // More parts may remain with unknown duration — use part index
                if i + 1 >= n_parts {
                    1.0
                } else {
                    ((i + 1) as f32 / n_parts as f32).clamp(0.0, 0.999)
                }
            } else {
                (done_bytes as f64 / total_after as f64).clamp(0.0, 0.999) as f32
            };

            manifest.tracks = track_metas.clone();
            manifest.tracks_done = (i + 1) as u32;
            manifest.bytes_downloaded = done_bytes;
            manifest.bytes_total = Some(total_after.max(done_bytes));
            manifest.progress = if i + 1 >= n_parts {
                1.0
            } else {
                progress_after
            };
            manifest.updated_at = now_rfc3339();
            last_emit_bytes = done_bytes;
            write_manifest(&manifest)?;
            emit_item(&app, &DownloadItem::from(&manifest));
        }

        // Sanity: we must have saved every part of the book playlist
        if track_metas.len() != n_parts {
            return Err(AppError::Message(format!(
                "download incomplete: got {} of {} parts",
                track_metas.len(),
                n_parts
            )));
        }

        manifest.status = DownloadStatus::Complete;
        manifest.progress = 1.0;
        manifest.error = None;
        manifest.bytes_total = Some(manifest.bytes_downloaded);
        manifest.downloaded_at = Some(now_rfc3339());
        manifest.updated_at = now_rfc3339();
        write_manifest(&manifest)?;
        let item = DownloadItem::from(&manifest);
        emit_item(&app, &item);
        Ok(item)
    }
    .await;

    manager.unregister(&rating_key);

    match result {
        Ok(item) => Ok(item),
        Err(e) => {
            let msg = e.to_string();
            let cancelled = msg.contains("cancelled");
            if let Ok(Some(mut m)) = read_manifest(&rating_key) {
                m.status = if cancelled {
                    DownloadStatus::Cancelled
                } else {
                    DownloadStatus::Error
                };
                m.error = Some(msg.clone());
                m.updated_at = now_rfc3339();
                let _ = write_manifest(&m);
                emit_item(&app, &DownloadItem::from(&m));
            }
            if cancelled {
                Err(AppError::Message("download cancelled".into()))
            } else {
                Err(e)
            }
        }
    }
}
