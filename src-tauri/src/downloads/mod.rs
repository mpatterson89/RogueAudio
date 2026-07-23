//! Offline audiobook downloads.
//!
//! Stores **original** library media parts (m4b/m4a/mp3/…) under
//! `~/.local/share/rogue-audio/downloads/{ratingKey}/` plus a manifest.json.
//! Online streaming still uses the MP3 transcoder for WebKit compatibility.

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
    /// Original container/extension (m4b, mp3, …).
    #[serde(default)]
    pub container: Option<String>,
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
    #[serde(default)]
    pub series: Option<String>,
    #[serde(default)]
    pub series_index: Option<u32>,
    /// Base file stem: Title-Series-Index-Author (no spaces).
    #[serde(default)]
    pub file_stem: Option<String>,
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
    pub series: Option<String>,
    pub series_index: Option<u32>,
    pub status: String,
    /// Whole-book progress 0.0 ..= 1.0
    pub progress: f32,
    pub error: Option<String>,
    pub tracks_done: u32,
    pub track_count: u32,
    pub bytes_downloaded: u64,
    pub bytes_total: Option<u64>,
    /// Actual bytes on disk under the book folder (audio + cover + manifest).
    pub bytes_on_disk: u64,
    pub duration_ms: Option<u64>,
    /// Absolute path to cover when present (frontend converts via convertFileSrc).
    pub cover_path: Option<String>,
    pub downloaded_at: Option<String>,
    /// Primary audio file name(s) for display.
    pub file_names: Vec<String>,
}

impl From<&DownloadManifest> for DownloadItem {
    fn from(m: &DownloadManifest) -> Self {
        let cover_path = m.cover_file.as_ref().and_then(|name| {
            book_dir(&m.rating_key)
                .ok()
                .map(|d| d.join(name).to_string_lossy().to_string())
        });
        let bytes_on_disk = dir_size(&m.rating_key).unwrap_or(m.bytes_downloaded);
        let file_names: Vec<String> = m
            .tracks
            .iter()
            .map(|t| t.file_name.clone())
            .collect();
        Self {
            rating_key: m.rating_key.clone(),
            server_id: m.server_id.clone(),
            title: m.title.clone(),
            author: m.author.clone(),
            series: m.series.clone(),
            series_index: m.series_index,
            status: m.status.as_str().to_string(),
            progress: m.progress,
            error: m.error.clone(),
            tracks_done: m.tracks_done,
            track_count: m.track_count,
            bytes_downloaded: m.bytes_downloaded,
            bytes_total: m.bytes_total,
            bytes_on_disk,
            duration_ms: m.duration_ms,
            cover_path,
            downloaded_at: m.downloaded_at.clone(),
            file_names,
        }
    }
}

/// Strip spaces/punctuation → alphanumeric slug for file names.
fn fs_slug(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// `{title}-{series?}-{index?}-{author?}` with no spaces.
pub fn book_file_stem(
    title: &str,
    series: Option<&str>,
    series_index: Option<u32>,
    author: Option<&str>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    let t = fs_slug(title);
    parts.push(if t.is_empty() {
        "Audiobook".into()
    } else {
        t
    });
    if let Some(s) = series.map(str::trim).filter(|s| !s.is_empty()) {
        let slug = fs_slug(s);
        if !slug.is_empty() {
            parts.push(slug);
        }
    }
    if let Some(i) = series_index.filter(|&n| n > 0) {
        parts.push(i.to_string());
    }
    if let Some(a) = author.map(str::trim).filter(|a| !a.is_empty()) {
        let slug = fs_slug(a);
        if !slug.is_empty() {
            parts.push(slug);
        }
    }
    parts.join("-")
}

fn audio_file_name(stem: &str, part_index: usize, part_count: usize, ext: &str) -> String {
    let ext = ext.trim_matches('.').to_ascii_lowercase();
    let ext = if ext.is_empty() { "bin" } else { ext.as_str() };
    if part_count <= 1 {
        format!("{stem}.{ext}")
    } else {
        format!("{stem}-part{:03}.{ext}", part_index + 1)
    }
}

fn dir_size(rating_key: &str) -> AppResult<u64> {
    let dir = downloads_root()?.join(safe_key(rating_key));
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut total = 0u64;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            total = total.saturating_add(entry.metadata()?.len());
        }
    }
    Ok(total)
}

/// After raw download as track_NNN.ext, rename to human-readable stem names.
fn rename_tracks_to_stem(
    dir: &Path,
    stem: &str,
    tracks: &mut [DownloadTrackMeta],
) -> AppResult<()> {
    let n = tracks.len();
    for (i, t) in tracks.iter_mut().enumerate() {
        let ext = t
            .container
            .as_deref()
            .or_else(|| {
                t.file_name
                    .rsplit('.')
                    .next()
                    .filter(|e| *e != t.file_name.as_str())
            })
            .unwrap_or("bin");
        let new_name = audio_file_name(stem, i, n, ext);
        if t.file_name == new_name {
            continue;
        }
        let from = dir.join(&t.file_name);
        let to = dir.join(&new_name);
        if from.exists() {
            if to.exists() && to != from {
                let _ = fs::remove_file(&to);
            }
            fs::rename(&from, &to)?;
            // Drop stale m4a playback hardlinks for old names
            let old_m4a = from.with_extension("m4a");
            if old_m4a.exists() {
                let _ = fs::remove_file(&old_m4a);
            }
        }
        t.file_name = new_name;
    }
    Ok(())
}

/// Fallback size estimate when Plex omits Part.size (~128 kbps AAC-ish).
const FALLBACK_BITRATE_BPS: f64 = 128_000.0;

fn estimate_bytes_from_duration(duration_ms: Option<u64>) -> Option<u64> {
    let ms = duration_ms.filter(|&d| d > 0)?;
    let secs = ms as f64 / 1000.0;
    Some((secs * FALLBACK_BITRATE_BPS / 8.0).round() as u64)
}

fn container_from_file_name(name: &str) -> String {
    name.rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| !e.is_empty() && e != name)
        .unwrap_or_else(|| "bin".into())
}

/// For containers WebKit handles poorly by extension, expose a playable alias.
fn playback_friendly_path(path: &Path, container: &str) -> PathBuf {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let needs_alias = ext == "m4b" || container.eq_ignore_ascii_case("m4b");
    if !needs_alias {
        return path.to_path_buf();
    }
    let alias = path.with_extension("m4a");
    if alias.exists() {
        return alias;
    }
    // Hard link shares the same inode — no extra disk use; keeps original .m4b name.
    if fs::hard_link(path, &alias).is_ok() {
        return alias;
    }
    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(path, &alias).is_ok() {
            return alias;
        }
    }
    path.to_path_buf()
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
    pub summary: Option<String>,
    pub year: Option<i32>,
    pub duration_ms: Option<u64>,
    pub library_key: Option<String>,
    pub studio: Option<String>,
    pub track_count: u32,
    pub server_id: String,
    /// Absolute path to cover.jpg when present.
    pub cover_path: Option<String>,
}

/// Build PlaybackInfo pointing at local original files (absolute paths in `url`).
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
        let container = t
            .container
            .clone()
            .unwrap_or_else(|| container_from_file_name(&t.file_name));
        // WebKit often won't map .m4b → audio/*; hardlink as .m4a for the asset URL only.
        // The on-disk original remains .m4b (or whatever was downloaded).
        let play_path = playback_friendly_path(&path, &container);
        tracks.push(PlaybackTrack {
            rating_key: t.rating_key.clone(),
            title: t.title.clone(),
            index: t.index,
            duration_ms: t.duration_ms,
            url: play_path.to_string_lossy().to_string(),
            container: Some(container),
        });
    }
    if tracks.is_empty() {
        return Ok(None);
    }
    let cover_path = m.cover_file.as_ref().map(|name| {
        dir.join(name).to_string_lossy().to_string()
    });
    Ok(Some(LocalPlayback {
        playback: PlaybackInfo {
            book_rating_key: m.rating_key.clone(),
            tracks,
            total_duration_ms: m.duration_ms,
        },
        chapters: m.chapters,
        title: m.title,
        author: m.author,
        summary: m.summary,
        year: m.year,
        duration_ms: m.duration_ms,
        library_key: m.library_key,
        studio: m.studio,
        track_count: m.track_count,
        server_id: m.server_id,
        cover_path,
    }))
}

pub fn remove_download(rating_key: &str) -> AppResult<()> {
    let dir = downloads_root()?.join(safe_key(rating_key));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

/// Delete every offline book folder under the downloads root.
pub fn remove_all_downloads() -> AppResult<u32> {
    let root = downloads_root()?;
    if !root.exists() {
        return Ok(0);
    }
    let mut n = 0u32;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
            n += 1;
        }
    }
    Ok(n)
}

/// Total bytes used by all offline books (for settings summary).
pub fn total_storage_bytes() -> AppResult<u64> {
    let root = downloads_root()?;
    if !root.exists() {
        return Ok(0);
    }
    let mut total = 0u64;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        for f in fs::read_dir(entry.path())? {
            let f = f?;
            if f.file_type()?.is_file() {
                total = total.saturating_add(f.metadata()?.len());
            }
        }
    }
    Ok(total)
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
        series: None,
        series_index: None,
        file_stem: None,
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
        // Full book: every original media part (m4b/mp3/…), not transcoder streams.
        let parts = plex::get_downloadable_parts(&server_id, &rating_key).await?;

        if parts.is_empty() {
            return Err(AppError::Message("no playable tracks to download".into()));
        }

        let total_duration_ms: Option<u64> = {
            let sum: u64 = parts.iter().filter_map(|p| p.duration_ms).sum();
            if sum > 0 {
                Some(sum)
            } else {
                detail.duration_ms
            }
        };

        manifest.title = detail.title.clone();
        manifest.author = detail.author.clone();
        manifest.summary = detail.summary.clone();
        manifest.year = detail.year;
        manifest.duration_ms = total_duration_ms.or(detail.duration_ms);
        manifest.library_key = detail.library_key.clone();
        manifest.studio = detail.studio.clone();
        manifest.series = detail.series.clone();
        manifest.series_index = detail.series_index;
        manifest.file_stem = Some(book_file_stem(
            &detail.title,
            detail.series.as_deref(),
            detail.series_index,
            detail.author.as_deref(),
        ));
        manifest.chapters = detail.chapters.clone();
        manifest.track_count = parts.len() as u32;
        // Prefer Plex Part.size for accurate whole-book progress
        let part_estimates: Vec<u64> = parts
            .iter()
            .map(|p| {
                p.size_bytes
                    .or_else(|| estimate_bytes_from_duration(p.duration_ms))
                    .unwrap_or(0)
            })
            .collect();
        let book_estimate: u64 = part_estimates.iter().sum();
        let book_estimate = if book_estimate > 0 {
            book_estimate
        } else {
            estimate_bytes_from_duration(manifest.duration_ms).unwrap_or(0)
        };
        manifest.bytes_total = (book_estimate > 0).then_some(book_estimate);
        manifest.updated_at = now_rfc3339();
        write_manifest(&manifest)?;
        emit_item(&app, &DownloadItem::from(&manifest));

        let client = download_http()?;

        // Cover is best-effort (does not affect book progress %)
        if let Some(cover) = try_download_cover(&client, &detail, &dir, &cancel).await {
            manifest.cover_file = Some(cover.clone());
            // Also land in the shared library cover cache
            let cover_path = dir.join(&cover);
            if let Err(e) = crate::covers::import_file(&server_id, &rating_key, &cover_path) {
                eprintln!("cover cache import failed: {e}");
            }
            write_manifest(&manifest)?;
        }

        let n_parts = parts.len();
        let mut track_metas: Vec<DownloadTrackMeta> = Vec::with_capacity(n_parts);
        let mut last_emit_bytes = 0u64;

        for (i, part) in parts.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                return Err(AppError::Message("download cancelled".into()));
            }

            let ext = part.file_ext.trim_matches('.').to_ascii_lowercase();
            let ext = if ext.is_empty() {
                "bin".to_string()
            } else {
                ext
            };
            let file_name = format!("track_{i:03}.{ext}");
            let dest = dir.join(&file_name);
            let track_index = i as u32;
            let completed_bytes: u64 = track_metas.iter().map(|t| t.bytes).sum();
            let future_estimates: Vec<u64> = parts[i + 1..]
                .iter()
                .map(|p| {
                    p.size_bytes
                        .or_else(|| estimate_bytes_from_duration(p.duration_ms))
                        .unwrap_or(0)
                })
                .collect();
            let size_est = part
                .size_bytes
                .or_else(|| estimate_bytes_from_duration(part.duration_ms));

            let bytes = download_url_to_file(
                &client,
                &part.url,
                &dest,
                &cancel,
                |n, content_length| {
                    // Prefer Content-Length, then Plex Part.size, then duration guess
                    let expected = content_length.or(size_est);
                    let (progress, total) =
                        whole_book_progress(completed_bytes, n, expected, &future_estimates);
                    manifest.progress = progress;
                    manifest.bytes_downloaded = completed_bytes.saturating_add(n);
                    manifest.bytes_total = Some(total);
                    manifest.tracks_done = i as u32; // current part not finished yet
                    manifest.updated_at = now_rfc3339();

                    // Throttle UI/disk: first chunk, every ~256 KiB
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
                rating_key: part.rating_key.clone(),
                title: part.title.clone(),
                duration_ms: part.duration_ms,
                file_name,
                bytes,
                container: Some(part.container.clone()),
            });

            let done_bytes: u64 = track_metas.iter().map(|t| t.bytes).sum();
            let remaining_est: u64 = future_estimates.iter().sum();
            let total_after = done_bytes.saturating_add(remaining_est).max(done_bytes);
            let progress_after = if remaining_est == 0 {
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

        // Sanity: we must have saved every part of the book
        if track_metas.len() != n_parts {
            return Err(AppError::Message(format!(
                "download incomplete: got {} of {} parts",
                track_metas.len(),
                n_parts
            )));
        }

        // Rename track_000.ext → Title-Series-1-Author.ext (no spaces)
        let stem = manifest
            .file_stem
            .clone()
            .unwrap_or_else(|| book_file_stem(&manifest.title, None, None, manifest.author.as_deref()));
        if let Err(e) = rename_tracks_to_stem(&dir, &stem, &mut track_metas) {
            eprintln!("rename download files failed: {e}");
        }
        manifest.file_stem = Some(stem);
        manifest.tracks = track_metas;
        manifest.bytes_downloaded = manifest.tracks.iter().map(|t| t.bytes).sum();

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
