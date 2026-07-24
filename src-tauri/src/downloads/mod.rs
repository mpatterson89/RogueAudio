//! Offline audiobook downloads with a persistent single-worker queue.
//!
//! Stores **original** library media parts (m4b/m4a/mp3/…) under
//! `~/.local/share/rogue-audio/downloads/{ratingKey}/` plus a manifest.json.
//! Queue pause/order lives in `download-queue.json`.
//! Online streaming still uses the MP3 transcoder for WebKit compatibility.

use crate::error::{AppError, AppResult};
use crate::plex::{self, BookChapter, BookDetail, PlaybackInfo, PlaybackTrack};
use crate::storage::app_data_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const MANIFEST_VERSION: u32 = 1;
const EVENT_PROGRESS: &str = "download-progress";
const EVENT_QUEUE: &str = "download-queue";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Complete,
    Error,
    Cancelled,
}

impl DownloadStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Paused => "paused",
            Self::Complete => "complete",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
        }
    }

    /// Still in the offline queue (not finished / cancelled).
    pub fn is_queue_member(&self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Downloading | Self::Paused | Self::Error
        )
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
    /// WebKit-safe MP3 sidecar (`*.play.mp3`) when the original is AAC/M4B/MP4.
    #[serde(default)]
    pub playable_file_name: Option<String>,
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
    /// Position in the download queue (0-based). None when not queued.
    #[serde(default)]
    pub queue_index: Option<u32>,
}

impl From<&DownloadManifest> for DownloadItem {
    fn from(m: &DownloadManifest) -> Self {
        let cover_path = m.cover_file.as_ref().and_then(|name| {
            book_dir(&m.rating_key)
                .ok()
                .map(|d| d.join(name).to_string_lossy().to_string())
        });
        let bytes_on_disk = dir_size(&m.rating_key).unwrap_or(m.bytes_downloaded);
        let file_names: Vec<String> = m.tracks.iter().map(|t| t.file_name.clone()).collect();
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
            queue_index: None,
        }
    }
}

/// Snapshot of the global download queue for the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueState {
    pub paused: bool,
    /// Items still in the queue (queued / downloading / paused / error), in order.
    pub order: Vec<String>,
    pub active_rating_key: Option<String>,
    /// Sum of estimated bytes for queue members (queued + downloading + paused + error).
    pub estimated_bytes: u64,
    /// Bytes already downloaded for those queue members.
    pub bytes_downloaded: u64,
    /// estimated_bytes − bytes_downloaded (remaining to pull).
    pub bytes_remaining: u64,
    pub queued_count: u32,
    pub active_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct QueuePersist {
    #[serde(default)]
    paused: bool,
    #[serde(default)]
    order: Vec<String>,
}

/// Per-job stop flags.
struct JobFlags {
    /// User cancelled this item (drop from queue; partials may be discarded).
    cancel: Arc<AtomicBool>,
    /// Queue paused — keep partials and mark status paused.
    pause: Arc<AtomicBool>,
}

struct ManagerInner {
    active: HashMap<String, JobFlags>,
    order: Vec<String>,
    paused: bool,
    /// True while a worker task is running (one at a time).
    running: bool,
}

impl Default for ManagerInner {
    fn default() -> Self {
        Self {
            active: HashMap::new(),
            order: Vec::new(),
            paused: false,
            running: false,
        }
    }
}

/// In-flight cancel/pause flags + ordered queue.
#[derive(Default)]
pub struct DownloadManager {
    inner: Mutex<ManagerInner>,
}

impl DownloadManager {
    fn with_inner<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut ManagerInner) -> R,
    {
        let mut guard = self.inner.lock().expect("download manager lock");
        f(&mut guard)
    }

    pub fn is_paused(&self) -> bool {
        self.with_inner(|i| i.paused)
    }

    pub fn is_active(&self, rating_key: &str) -> bool {
        self.with_inner(|i| i.active.contains_key(rating_key))
    }

    pub fn active_rating_key(&self) -> Option<String> {
        self.with_inner(|i| i.active.keys().next().cloned())
    }

    pub fn order(&self) -> Vec<String> {
        self.with_inner(|i| i.order.clone())
    }

    fn request_cancel_item(&self, rating_key: &str) {
        self.with_inner(|i| {
            if let Some(flags) = i.active.get(rating_key) {
                flags.cancel.store(true, Ordering::SeqCst);
            }
        });
    }

    /// Claim the single worker slot for `rating_key`.
    fn try_begin(&self, rating_key: &str) -> Option<JobFlags> {
        self.with_inner(|i| {
            if i.running || i.paused {
                return None;
            }
            if i.active.contains_key(rating_key) {
                return None;
            }
            let flags = JobFlags {
                cancel: Arc::new(AtomicBool::new(false)),
                pause: Arc::new(AtomicBool::new(false)),
            };
            i.active.insert(
                rating_key.to_string(),
                JobFlags {
                    cancel: Arc::clone(&flags.cancel),
                    pause: Arc::clone(&flags.pause),
                },
            );
            i.running = true;
            Some(flags)
        })
    }

    fn unregister(&self, rating_key: &str) {
        self.with_inner(|i| {
            i.active.remove(rating_key);
            i.running = false;
        });
    }

    fn persist_queue_locked(inner: &ManagerInner) {
        let data = QueuePersist {
            paused: inner.paused,
            order: inner.order.clone(),
        };
        let _ = write_queue_persist(&data);
    }

    fn ensure_in_order_locked(inner: &mut ManagerInner, rating_key: &str) {
        if !inner.order.iter().any(|k| k == rating_key) {
            inner.order.push(rating_key.to_string());
        }
    }

    fn remove_from_order_locked(inner: &mut ManagerInner, rating_key: &str) {
        inner.order.retain(|k| k != rating_key);
    }
}

fn queue_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("download-queue.json"))
}

fn read_queue_persist() -> QueuePersist {
    let Ok(path) = queue_path() else {
        return QueuePersist::default();
    };
    if !path.exists() {
        return QueuePersist::default();
    }
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn write_queue_persist(data: &QueuePersist) -> AppResult<()> {
    let path = queue_path()?;
    let raw = serde_json::to_string_pretty(data)
        .map_err(|e| AppError::Message(e.to_string()))?;
    fs::write(path, raw)?;
    Ok(())
}

/// Strip spaces/punctuation → alphanumeric slug for file names.
fn fs_slug(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect()
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
/// Only updates `file_name` when the target path actually exists afterward.
fn rename_tracks_to_stem(
    dir: &Path,
    stem: &str,
    tracks: &mut [DownloadTrackMeta],
) -> AppResult<()> {
    let n = tracks.len();
    for (i, t) in tracks.iter_mut().enumerate() {
        // Prefer the on-disk extension (m4b) over Plex Media.container (often "mp4"
        // for AAC audiobooks) — renaming m4b→mp4 breaks WebKit playback.
        let ext = t
            .file_name
            .rsplit('.')
            .next()
            .filter(|e| *e != t.file_name.as_str() && !e.is_empty())
            .or(t.container.as_deref())
            .unwrap_or("bin");
        let new_name = audio_file_name(stem, i, n, ext);
        if t.file_name == new_name {
            continue;
        }
        let from = dir.join(&t.file_name);
        let to = dir.join(&new_name);
        if to.exists() && to != from {
            // Already renamed on a previous attempt
            if from.exists() {
                let _ = fs::remove_file(&from);
            }
            t.file_name = new_name;
            continue;
        }
        if !from.exists() {
            // Keep the old name if we cannot find the bytes
            continue;
        }
        if let Err(e) = fs::rename(&from, &to) {
            // Cross-device / busy: fall back to copy
            if let Err(e2) = fs::copy(&from, &to).and_then(|_| fs::remove_file(&from)) {
                return Err(AppError::Message(format!(
                    "rename download file {} → {}: {e}; copy fallback: {e2}",
                    from.display(),
                    to.display()
                )));
            }
        }
        let old_m4a = from.with_extension("m4a");
        if old_m4a.exists() {
            let _ = fs::remove_file(&old_m4a);
        }
        t.file_name = new_name;
    }
    Ok(())
}

fn partial_path_for(dest: &Path) -> PathBuf {
    // Avoid Path::with_extension which can surprise on multi-dot names.
    // Always append ".part" so track_000.mp4 → track_000.mp4.part
    let mut name = dest
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_else(|| "download".into());
    name.push(".part");
    dest.with_file_name(name)
}

fn is_audio_file_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".part")
        || lower.ends_with(".partial")
        || lower.ends_with(".play.mp3")
        || lower.ends_with(".play.mp3.part")
    {
        return false;
    }
    // Skip hardlink aliases created for WebKit (prefer original in heal)
    // Heal uses unique originals; .m4a next to .m4b/.mp4 is usually an alias.
    let ext = lower
        .rsplit('.')
        .next()
        .unwrap_or_default();
    matches!(
        ext,
        "m4b" | "m4a" | "mp3" | "mp4" | "aac" | "flac" | "ogg" | "opus" | "wma" | "m4v"
    )
}

/// If a download left media on disk but never wrote track metadata (error at 99%),
/// promote the folder to complete so offline playback works.
pub fn heal_incomplete_manifest(rating_key: &str) -> AppResult<Option<DownloadManifest>> {
    let Some(mut m) = read_manifest(rating_key)? else {
        return Ok(None);
    };
    if m.status == DownloadStatus::Complete && !m.tracks.is_empty() {
        // Verify files still exist
        let dir = book_dir(rating_key)?;
        let ok = m.tracks.iter().all(|t| dir.join(&t.file_name).is_file());
        if ok {
            return Ok(Some(m));
        }
    }

    // Only heal error / interrupted states that look essentially finished
    let candidates = matches!(
        m.status,
        DownloadStatus::Error
            | DownloadStatus::Downloading
            | DownloadStatus::Paused
            | DownloadStatus::Queued
            | DownloadStatus::Cancelled
    );
    if !candidates {
        return Ok(Some(m));
    }

    let dir = book_dir(rating_key)?;
    let mut audio: Vec<(String, u64)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "manifest.json" || name == "cover.jpg" {
                continue;
            }
            if !is_audio_file_name(&name) {
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() && meta.len() > 0 {
                    audio.push((name, meta.len()));
                }
            }
        }
    }
    if audio.is_empty() {
        return Ok(Some(m));
    }
    audio.sort_by(|a, b| a.0.cmp(&b.0));

    // Heal only when tracks metadata is broken/missing AND the file looks finished.
    // Do not promote short / interrupted pulls to "complete" (was marking ~50% as done).
    let tracks_missing = m.tracks.is_empty()
        || m.tracks
            .iter()
            .any(|t| !dir.join(&t.file_name).is_file());
    let on_disk: u64 = audio.iter().map(|(_, n)| *n).sum();
    let expected = m.bytes_total.filter(|&t| t > 0);
    let size_ok = match expected {
        // Within 1% or 256 KiB of declared total
        Some(exp) => {
            let slack = (exp / 100).max(256 * 1024);
            on_disk + slack >= exp && on_disk > 0
        }
        // No size hint: require high progress
        None => m.progress >= 0.99 && on_disk > 1024 * 1024,
    };
    if !(tracks_missing && size_ok) {
        return Ok(Some(m));
    }

    let total: u64 = audio.iter().map(|(_, n)| *n).sum();
    m.tracks = audio
        .into_iter()
        .enumerate()
        .map(|(i, (file_name, bytes))| {
            let container = container_from_file_name(&file_name);
            DownloadTrackMeta {
                index: i as u32,
                rating_key: m.rating_key.clone(),
                title: if i == 0 {
                    m.title.clone()
                } else {
                    format!("{} — part {}", m.title, i + 1)
                },
                duration_ms: None,
                file_name,
                bytes,
                container: Some(container),
                playable_file_name: None,
            }
        })
        .collect();
    m.track_count = m.tracks.len() as u32;
    m.tracks_done = m.track_count;
    m.bytes_downloaded = total;
    m.bytes_total = Some(total);
    m.progress = 1.0;
    m.status = DownloadStatus::Complete;
    m.error = None;
    m.downloaded_at = m.downloaded_at.or_else(|| Some(now_rfc3339()));
    m.updated_at = now_rfc3339();
    if m.cover_file.is_none() && dir.join("cover.jpg").is_file() {
        m.cover_file = Some("cover.jpg".into());
    }
    write_manifest(&m)?;
    Ok(Some(m))
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

/// Extensions WebKit/HTMLAudio can usually play without a native AAC stack.
fn is_web_safe_audio_ext(ext: &str) -> bool {
    matches!(ext, "mp3" | "mpeg" | "ogg" | "opus" | "wav" | "flac")
}

fn file_ext_lower(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Sidecar next to the original: `Book.m4b` → `Book.play.mp3`.
fn playable_sidecar_path(original: &Path) -> PathBuf {
    let stem = original
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".into());
    original.with_file_name(format!("{stem}.play.mp3"))
}

fn needs_playable_transcode(path: &Path, container: &str) -> bool {
    let ext = file_ext_lower(path);
    if is_web_safe_audio_ext(&ext) {
        return false;
    }
    let c = container.to_ascii_lowercase();
    // mp3 already safe; treat empty as needing check by extension only
    if is_web_safe_audio_ext(&c) {
        return false;
    }
    true
}

/// Resolve the best path for HTML5 playback (never blocks on multi‑GB copies).
fn resolve_playable_path(path: &Path, container: &str, playable_name: Option<&str>) -> PathBuf {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    if let Some(name) = playable_name {
        let p = dir.join(name);
        if p.is_file() {
            return p;
        }
    }
    let sidecar = playable_sidecar_path(path);
    if sidecar.is_file() {
        return sidecar;
    }
    let ext = file_ext_lower(path);
    if is_web_safe_audio_ext(&ext) {
        return path.to_path_buf();
    }
    // Cheap hardlink/symlink to .m4a — helps when WebKit has AAC + correct MIME.
    // Never fs::copy: copying 0.5–1 GB on the IPC thread freezes the UI.
    if matches!(ext.as_str(), "m4b" | "mp4" | "m4v" | "m4a")
        || matches!(
            container.to_ascii_lowercase().as_str(),
            "m4b" | "mp4" | "aac" | "m4a" | "mp4a"
        )
    {
        let alias = path.with_extension("m4a");
        if alias.is_file() {
            return alias;
        }
        if path.is_file() {
            if fs::hard_link(path, &alias).is_ok() {
                return alias;
            }
            #[cfg(unix)]
            {
                if std::os::unix::fs::symlink(path, &alias).is_ok() {
                    return alias;
                }
            }
        }
    }
    path.to_path_buf()
}

/// Transcode original → `.play.mp3` with ffmpeg so WebKit can play offline AAC/M4B.
fn transcode_to_playable_mp3(src: &Path, dest: &Path) -> AppResult<()> {
    if !src.is_file() {
        return Err(AppError::Message(format!(
            "source missing for transcode: {}",
            src.display()
        )));
    }
    if dest.is_file() {
        if let (Ok(s), Ok(d)) = (fs::metadata(src), fs::metadata(dest)) {
            // Sidecar already present and non-trivial
            if d.len() > 1024 * 64 && d.len() as f64 > s.len() as f64 * 0.15 {
                return Ok(());
            }
        }
    }
    // Append .part (do not use with_extension — it replaces ".mp3" incorrectly)
    let tmp = {
        let mut p = dest.as_os_str().to_os_string();
        p.push(".part");
        PathBuf::from(p)
    };
    let _ = fs::remove_file(&tmp);

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-y",
            "-i",
        ])
        .arg(src)
        .args([
            "-vn",
            "-c:a",
            "libmp3lame",
            "-q:a",
            "4", // VBR ~165kbps — faster encode, still fine for speech
            "-map_metadata",
            "0",
            "-id3v2_version",
            "3",
            "-threads",
            "0",
        ])
        .arg(&tmp)
        .status()
        .map_err(|e| {
            AppError::Message(format!(
                "ffmpeg failed to start (is it installed?): {e}"
            ))
        })?;

    if !status.success() {
        let _ = fs::remove_file(&tmp);
        return Err(AppError::Message(format!(
            "ffmpeg transcode failed for {}",
            src.display()
        )));
    }
    let len = fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
    if len < 1024 {
        let _ = fs::remove_file(&tmp);
        return Err(AppError::Message("ffmpeg produced empty mp3".into()));
    }
    if dest.exists() {
        let _ = fs::remove_file(dest);
    }
    fs::rename(&tmp, dest)?;
    Ok(())
}

/// Ensure each track has a WebKit-playable file. Idempotent.
pub fn ensure_playable(rating_key: &str) -> AppResult<DownloadItem> {
    let Some(mut m) = read_manifest(rating_key)? else {
        return Err(AppError::Message("download not found".into()));
    };
    if m.status != DownloadStatus::Complete {
        return Err(AppError::Message(
            "download is not complete — finish or resume it first".into(),
        ));
    }
    let dir = book_dir(rating_key)?;
    let mut changed = false;
    for t in m.tracks.iter_mut() {
        let original = dir.join(&t.file_name);
        if !original.is_file() {
            return Err(AppError::Message(format!(
                "missing audio file: {}",
                t.file_name
            )));
        }
        let container = t
            .container
            .clone()
            .unwrap_or_else(|| container_from_file_name(&t.file_name));
        if !needs_playable_transcode(&original, &container) {
            // Already web-safe (e.g. mp3)
            if t.playable_file_name.is_some() {
                t.playable_file_name = None;
                changed = true;
            }
            continue;
        }
        let sidecar = playable_sidecar_path(&original);
        let sidecar_name = sidecar
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("{}.play.mp3", t.index));
        if !sidecar.is_file() {
            transcode_to_playable_mp3(&original, &sidecar)?;
        }
        if t.playable_file_name.as_deref() != Some(sidecar_name.as_str()) {
            t.playable_file_name = Some(sidecar_name);
            changed = true;
        }
    }
    if changed {
        m.updated_at = now_rfc3339();
        write_manifest(&m)?;
    }
    Ok(DownloadItem::from(&m))
}

/// Whether every complete track has a web-safe path ready (no ffmpeg needed).
pub fn playable_ready(rating_key: &str) -> bool {
    let Ok(Some(m)) = read_manifest(rating_key) else {
        return false;
    };
    if m.status != DownloadStatus::Complete || m.tracks.is_empty() {
        return false;
    }
    let Ok(dir) = book_dir(rating_key) else {
        return false;
    };
    m.tracks.iter().all(|t| {
        let original = dir.join(&t.file_name);
        if !original.is_file() {
            return false;
        }
        let container = t
            .container
            .clone()
            .unwrap_or_else(|| container_from_file_name(&t.file_name));
        if !needs_playable_transcode(&original, &container) {
            return true;
        }
        if let Some(name) = &t.playable_file_name {
            if dir.join(name).is_file() {
                return true;
            }
        }
        playable_sidecar_path(&original).is_file()
    })
}

/// Whole-book progress from completed parts + the in-flight part.
fn whole_book_progress(
    completed_bytes: u64,
    current_bytes: u64,
    current_expected: Option<u64>,
    future_part_estimates: &[u64],
) -> (f32, u64) {
    let current_total = match current_expected {
        Some(expected) if expected > current_bytes => expected,
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
    let raw =
        serde_json::to_string_pretty(m).map_err(|e| AppError::Message(e.to_string()))?;
    fs::write(path, raw)?;
    Ok(())
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn emit_item(app: &AppHandle, item: &DownloadItem) {
    let _ = app.emit(EVENT_PROGRESS, item);
}

fn emit_queue(app: &AppHandle, state: &QueueState) {
    let _ = app.emit(EVENT_QUEUE, state);
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
        // Heal "99% + file on disk but error/empty tracks" leftovers
        let m = match heal_incomplete_manifest(&m.rating_key) {
            Ok(Some(healed)) => healed,
            _ => m,
        };
        out.push(DownloadItem::from(&m));
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(out)
}

pub fn get_download(rating_key: &str) -> AppResult<Option<DownloadItem>> {
    if let Ok(Some(m)) = heal_incomplete_manifest(rating_key) {
        return Ok(Some(DownloadItem::from(&m)));
    }
    Ok(read_manifest(rating_key)?.as_ref().map(DownloadItem::from))
}

/// Build queue totals from on-disk manifests + manager order.
pub fn compute_queue_state(manager: &DownloadManager) -> QueueState {
    let paused = manager.is_paused();
    let order = manager.order();
    let active = manager.active_rating_key();

    let mut estimated_bytes = 0u64;
    let mut bytes_downloaded = 0u64;
    let mut queued_count = 0u32;
    let mut active_count = 0u32;

    for key in &order {
        let Ok(Some(m)) = read_manifest(key) else {
            continue;
        };
        if !m.status.is_queue_member() {
            continue;
        }
        let total = m
            .bytes_total
            .or_else(|| estimate_bytes_from_duration(m.duration_ms))
            .unwrap_or(0);
        let done = m.bytes_downloaded.min(if total > 0 { total } else { m.bytes_downloaded });
        estimated_bytes = estimated_bytes.saturating_add(total.max(done));
        bytes_downloaded = bytes_downloaded.saturating_add(done);
        match m.status {
            DownloadStatus::Downloading => active_count = active_count.saturating_add(1),
            DownloadStatus::Queued | DownloadStatus::Paused | DownloadStatus::Error => {
                queued_count = queued_count.saturating_add(1);
            }
            _ => {}
        }
    }

    let bytes_remaining = estimated_bytes.saturating_sub(bytes_downloaded);

    QueueState {
        paused,
        order,
        active_rating_key: active,
        estimated_bytes,
        bytes_downloaded,
        bytes_remaining,
        queued_count,
        active_count,
    }
}

pub fn queue_state(manager: &DownloadManager) -> QueueState {
    compute_queue_state(manager)
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

/// Build PlaybackInfo pointing at local files.
/// Prefers WebKit-safe `.play.mp3` sidecars and serves via the local media HTTP server
/// (asset: protocol is unreliable for large audiobook files in WebKitGTK).
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
        let play_path =
            resolve_playable_path(&path, &container, t.playable_file_name.as_deref());
        let play_container = file_ext_lower(&play_path);
        // Prefer localhost media server so HTML5 gets Range + correct MIME.
        // Fall back to absolute path for convertFileSrc if server is down.
        let url = crate::media_server::url_for_download_file(&play_path)
            .unwrap_or_else(|_| play_path.to_string_lossy().into_owned());
        tracks.push(PlaybackTrack {
            rating_key: t.rating_key.clone(),
            title: t.title.clone(),
            index: t.index,
            duration_ms: t.duration_ms,
            url,
            container: Some(if play_container.is_empty() {
                container
            } else {
                play_container
            }),
        });
    }
    if tracks.is_empty() {
        return Ok(None);
    }
    let cover_path = m
        .cover_file
        .as_ref()
        .map(|name| dir.join(name).to_string_lossy().to_string());
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

#[derive(Debug)]
enum StopKind {
    Cancel,
    Pause,
}

fn check_stop(cancel: &AtomicBool, pause: &AtomicBool) -> Result<(), StopKind> {
    if cancel.load(Ordering::SeqCst) {
        return Err(StopKind::Cancel);
    }
    if pause.load(Ordering::SeqCst) {
        return Err(StopKind::Pause);
    }
    Ok(())
}

/// Promote a temp/partial file to `dest`. Tolerates already-finalized files.
fn promote_partial_to_dest(tmp: &Path, dest: &Path) -> AppResult<u64> {
    if dest.is_file() {
        let len = fs::metadata(dest)?.len();
        if len > 0 {
            if tmp.exists() {
                let _ = fs::remove_file(tmp);
            }
            return Ok(len);
        }
    }
    if !tmp.is_file() {
        return Err(AppError::Message(format!(
            "download temp missing: {}",
            tmp.display()
        )));
    }
    let len = fs::metadata(tmp)?.len();
    if len == 0 {
        let _ = fs::remove_file(tmp);
        return Err(AppError::Message("download produced empty file".into()));
    }
    if dest.exists() {
        let _ = fs::remove_file(dest);
    }
    match fs::rename(tmp, dest) {
        Ok(()) => Ok(fs::metadata(dest).map(|m| m.len()).unwrap_or(len)),
        Err(e) => {
            // rename can fail across filesystems or if dest is busy — copy instead
            fs::copy(tmp, dest).map_err(|e2| {
                AppError::Message(format!(
                    "finalize download {} → {}: rename: {e}; copy: {e2}",
                    tmp.display(),
                    dest.display()
                ))
            })?;
            let _ = fs::remove_file(tmp);
            Ok(fs::metadata(dest).map(|m| m.len()).unwrap_or(len))
        }
    }
}

/// Download a URL to `dest`, resuming from an existing `.part` when possible.
/// `on_progress(downloaded_including_resume, full_content_length_opt)` is called
/// as chunks arrive.
async fn download_url_to_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    cancel: &AtomicBool,
    pause: &AtomicBool,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<u64, AppError> {
    // Already finished on a previous run
    if dest.is_file() {
        match fs::metadata(dest) {
            Ok(meta) if meta.len() > 0 => {
                let len = meta.len();
                on_progress(len, Some(len));
                return Ok(len);
            }
            _ => {}
        }
    }

    // Prefer new ".part" suffix; also resume legacy ".partial" extension temps
    let tmp = partial_path_for(dest);
    let legacy_tmp = dest.with_extension("partial");
    if !tmp.exists() && legacy_tmp.exists() {
        let _ = fs::rename(&legacy_tmp, &tmp);
    }

    let mut existing = if tmp.is_file() {
        fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    let mut req = client.get(url);
    if existing > 0 {
        req = req.header("Range", format!("bytes={existing}-"));
    }

    let res = req
        .send()
        .await
        .map_err(|e| AppError::Message(format!("download request failed: {e}")))?;

    let status = res.status();
    if status.as_u16() == 416 && existing > 0 {
        // Range not satisfiable — partial may already be complete
        if let Err(e) = check_stop(cancel, pause).map_err(stop_to_err) {
            return Err(e);
        }
        let len = promote_partial_to_dest(&tmp, dest)?;
        on_progress(len, Some(len));
        return Ok(len);
    }

    if !status.is_success() {
        return Err(AppError::Message(format!(
            "download HTTP {}: {}",
            status,
            dest.display()
        )));
    }

    // 200 with a Range request ⇒ server ignored Range; restart from 0
    let is_partial = status.as_u16() == 206;
    if existing > 0 && !is_partial {
        existing = 0;
        let _ = fs::remove_file(&tmp);
    }

    let content_length = res.content_length().filter(|&n| n > 0);
    let full_expected = if is_partial {
        // Content-Length is the remaining body size
        content_length.map(|n| n.saturating_add(existing))
    } else {
        content_length
    };

    on_progress(existing, full_expected);

    let mut file = if existing > 0 && is_partial && tmp.is_file() {
        match fs::OpenOptions::new().append(true).open(&tmp) {
            Ok(mut f) => {
                let _ = f.seek(SeekFrom::End(0));
                f
            }
            Err(_) => {
                // Partial vanished between stat and open — start over
                existing = 0;
                on_progress(0, full_expected);
                fs::File::create(&tmp)?
            }
        }
    } else {
        fs::File::create(&tmp)?
    };

    let mut total = existing;
    let mut stream = res;
    loop {
        if let Err(kind) = check_stop(cancel, pause) {
            drop(file);
            // Keep partial for pause/resume; discard on cancel
            if matches!(kind, StopKind::Cancel) {
                let _ = fs::remove_file(&tmp);
            }
            return Err(stop_to_err(kind));
        }
        match stream.chunk().await {
            Ok(Some(chunk)) => {
                if let Err(e) = file.write_all(&chunk) {
                    drop(file);
                    return Err(AppError::Message(format!(
                        "download write error ({}): {e}",
                        tmp.display()
                    )));
                }
                total += chunk.len() as u64;
                on_progress(total, full_expected);
            }
            Ok(None) => break,
            Err(e) => {
                drop(file);
                // Keep partial so the next attempt can resume after network blips
                return Err(AppError::Message(format!("download stream error: {e}")));
            }
        }
    }
    if let Err(e) = file.flush() {
        drop(file);
        return Err(AppError::Message(format!(
            "download flush error ({}): {e}",
            tmp.display()
        )));
    }
    drop(file);

    if total == 0 {
        let _ = fs::remove_file(&tmp);
        return Err(AppError::Message("download produced empty file".into()));
    }

    let final_len = promote_partial_to_dest(&tmp, dest)?;
    Ok(final_len)
}

fn stop_to_err(kind: StopKind) -> AppError {
    match kind {
        StopKind::Cancel => AppError::Message("download cancelled".into()),
        StopKind::Pause => AppError::Message("download paused".into()),
    }
}

async fn try_download_cover(
    client: &reqwest::Client,
    detail: &BookDetail,
    dir: &Path,
    cancel: &AtomicBool,
    pause: &AtomicBool,
) -> Option<String> {
    let url = detail.thumb.as_ref().or(detail.art.as_ref())?;
    if check_stop(cancel, pause).is_err() {
        return None;
    }
    let dest = dir.join("cover.jpg");
    if dest.exists() {
        return Some("cover.jpg".into());
    }
    match download_url_to_file(client, url, &dest, cancel, pause, |_, _| {}).await {
        Ok(_) => Some("cover.jpg".into()),
        Err(_) => None,
    }
}

/// Enqueue a book. Does not start work when the queue is paused or busy.
pub fn enqueue(
    app: &AppHandle,
    manager: &Arc<DownloadManager>,
    server_id: String,
    rating_key: String,
) -> AppResult<DownloadItem> {
    if let Some(m) = read_manifest(&rating_key)? {
        if m.status == DownloadStatus::Complete {
            return Ok(DownloadItem::from(&m));
        }
    }

    // Seed / refresh a queued manifest without wiping resume progress
    let mut manifest = if let Some(existing) = read_manifest(&rating_key)? {
        existing
    } else {
        DownloadManifest {
            version: MANIFEST_VERSION,
            rating_key: rating_key.clone(),
            server_id: server_id.clone(),
            title: "Queued…".into(),
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
            status: DownloadStatus::Queued,
            progress: 0.0,
            error: None,
            tracks_done: 0,
            track_count: 0,
            bytes_downloaded: 0,
            bytes_total: None,
            downloaded_at: None,
            updated_at: now_rfc3339(),
        }
    };

    // Preserve partial progress; re-queue from error/cancelled/paused
    if manifest.status != DownloadStatus::Downloading || !manager.is_active(&rating_key) {
        if manifest.status != DownloadStatus::Complete {
            manifest.status = if manager.is_paused() {
                DownloadStatus::Paused
            } else {
                DownloadStatus::Queued
            };
            manifest.error = None;
            manifest.server_id = server_id;
            manifest.updated_at = now_rfc3339();
            write_manifest(&manifest)?;
        }
    }

    manager.with_inner(|inner| {
        DownloadManager::ensure_in_order_locked(inner, &rating_key);
        DownloadManager::persist_queue_locked(inner);
    });

    let mut item = DownloadItem::from(&manifest);
    let order = manager.order();
    item.queue_index = order
        .iter()
        .position(|k| k == &rating_key)
        .map(|i| i as u32);
    emit_item(app, &item);
    emit_queue(app, &compute_queue_state(manager));

    pump_queue(app.clone(), Arc::clone(manager));

    Ok(item)
}

/// Cancel one item (remove from queue; stop if active).
pub fn cancel_item(app: &AppHandle, manager: &Arc<DownloadManager>, rating_key: &str) -> AppResult<()> {
    manager.with_inner(|inner| {
        DownloadManager::remove_from_order_locked(inner, rating_key);
        DownloadManager::persist_queue_locked(inner);
        if let Some(flags) = inner.active.get(rating_key) {
            flags.cancel.store(true, Ordering::SeqCst);
        }
    });

    // If not actively running, mark cancelled immediately
    if !manager.is_active(rating_key) {
        if let Ok(Some(mut m)) = read_manifest(rating_key) {
            if m.status.is_queue_member() {
                m.status = DownloadStatus::Cancelled;
                m.error = None;
                m.updated_at = now_rfc3339();
                let _ = write_manifest(&m);
                emit_item(app, &DownloadItem::from(&m));
            }
        }
    }

    emit_queue(app, &compute_queue_state(manager));
    // If we cancelled a waiting item, try to start the next
    pump_queue(app.clone(), Arc::clone(manager));
    Ok(())
}

/// Pause the whole queue; in-flight job keeps partials.
pub fn pause_queue(app: &AppHandle, manager: &Arc<DownloadManager>) -> AppResult<QueueState> {
    manager.with_inner(|inner| {
        inner.paused = true;
        for flags in inner.active.values() {
            flags.pause.store(true, Ordering::SeqCst);
        }
        DownloadManager::persist_queue_locked(inner);
    });

    // Mark non-active queue members as paused for UI consistency
    let order = manager.order();
    for key in order {
        if manager.is_active(&key) {
            continue;
        }
        if let Ok(Some(mut m)) = read_manifest(&key) {
            if matches!(
                m.status,
                DownloadStatus::Queued | DownloadStatus::Downloading
            ) {
                m.status = DownloadStatus::Paused;
                m.updated_at = now_rfc3339();
                let _ = write_manifest(&m);
                emit_item(app, &DownloadItem::from(&m));
            }
        }
    }

    let state = compute_queue_state(manager);
    emit_queue(app, &state);
    Ok(state)
}

/// Resume the queue and start the next job if idle.
pub fn resume_queue(app: &AppHandle, manager: &Arc<DownloadManager>) -> AppResult<QueueState> {
    manager.with_inner(|inner| {
        inner.paused = false;
        DownloadManager::persist_queue_locked(inner);
    });

    let order = manager.order();
    for key in order {
        if let Ok(Some(mut m)) = read_manifest(&key) {
            if matches!(
                m.status,
                DownloadStatus::Paused | DownloadStatus::Error | DownloadStatus::Downloading
            ) && !manager.is_active(&key)
            {
                m.status = DownloadStatus::Queued;
                m.error = None;
                m.updated_at = now_rfc3339();
                let _ = write_manifest(&m);
                emit_item(app, &DownloadItem::from(&m));
            }
        }
    }

    let state = compute_queue_state(manager);
    emit_queue(app, &state);
    pump_queue(app.clone(), Arc::clone(manager));
    Ok(state)
}

/// Cold-start: reload persisted queue, heal interrupted jobs, maybe auto-resume.
pub fn restore_queue(app: &AppHandle, manager: &Arc<DownloadManager>) -> AppResult<QueueState> {
    let persist = read_queue_persist();

    manager.with_inner(|inner| {
        inner.paused = persist.paused;
        inner.order = persist.order;
        inner.active.clear();
        inner.running = false;
    });

    // Heal any "downloading" left from a crash / force-quit
    let mut found: Vec<String> = Vec::new();
    if let Ok(items) = list_downloads() {
        for item in items {
            let Ok(Some(mut m)) = read_manifest(&item.rating_key) else {
                continue;
            };
            match m.status {
                DownloadStatus::Downloading => {
                    m.status = if persist.paused {
                        DownloadStatus::Paused
                    } else {
                        DownloadStatus::Queued
                    };
                    m.updated_at = now_rfc3339();
                    let _ = write_manifest(&m);
                    emit_item(app, &DownloadItem::from(&m));
                    found.push(m.rating_key);
                }
                DownloadStatus::Queued | DownloadStatus::Paused | DownloadStatus::Error => {
                    found.push(m.rating_key);
                }
                _ => {}
            }
        }
    }

    manager.with_inner(|inner| {
        // Keep persisted order; append any healed keys that were missing
        for key in found {
            if !inner.order.iter().any(|k| k == &key) {
                inner.order.push(key);
            }
        }
        // Drop order entries that no longer exist or are complete/cancelled
        inner.order.retain(|k| {
            read_manifest(k)
                .ok()
                .flatten()
                .map(|m| m.status.is_queue_member())
                .unwrap_or(false)
        });
        DownloadManager::persist_queue_locked(inner);
    });

    let state = compute_queue_state(manager);
    emit_queue(app, &state);

    if !persist.paused {
        pump_queue(app.clone(), Arc::clone(manager));
    }

    Ok(state)
}

/// Start the next queued job when the worker is free and the queue is not paused.
pub fn pump_queue(app: AppHandle, manager: Arc<DownloadManager>) {
    let next = manager.with_inner(|inner| {
        if inner.paused || inner.running {
            return None;
        }
        // Only auto-start ready items. Errors wait for resume/retry; paused waits for resume.
        for key in inner.order.clone() {
            if let Ok(Some(m)) = read_manifest(&key) {
                let ready = matches!(
                    m.status,
                    DownloadStatus::Queued | DownloadStatus::Downloading
                );
                if ready {
                    return Some((m.server_id, key));
                }
            }
        }
        None
    });

    let Some((server_id, rating_key)) = next else {
        return;
    };

    let Some(flags) = manager.try_begin(&rating_key) else {
        return;
    };

    let mgr = Arc::clone(&manager);
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = run_download(
            app2.clone(),
            Arc::clone(&mgr),
            flags,
            server_id,
            rating_key.clone(),
        )
        .await;
        if let Err(e) = result {
            eprintln!("download {rating_key} failed: {e}");
        }
        // Continue with next item when this one finishes (unless paused)
        pump_queue(app2, mgr);
    });
}

/// Run the full download job. Emits `download-progress` events.
async fn run_download(
    app: AppHandle,
    manager: Arc<DownloadManager>,
    flags: JobFlags,
    server_id: String,
    rating_key: String,
) -> AppResult<DownloadItem> {
    let cancel = flags.cancel;
    let pause = flags.pause;

    // Already complete — no-op
    if let Some(m) = read_manifest(&rating_key)? {
        if m.status == DownloadStatus::Complete {
            manager.with_inner(|inner| {
                DownloadManager::remove_from_order_locked(inner, &rating_key);
                DownloadManager::persist_queue_locked(inner);
            });
            manager.unregister(&rating_key);
            let item = DownloadItem::from(&m);
            emit_item(&app, &item);
            emit_queue(&app, &compute_queue_state(&manager));
            return Ok(item);
        }
    }

    let dir = book_dir(&rating_key)?;

    // Resume from existing manifest when present
    let mut manifest = if let Some(existing) = read_manifest(&rating_key)? {
        existing
    } else {
        DownloadManifest {
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
        }
    };

    manifest.status = DownloadStatus::Downloading;
    manifest.error = None;
    manifest.server_id = server_id.clone();
    if manifest.title.is_empty() || manifest.title == "Queued…" {
        manifest.title = "Downloading…".into();
    }
    manifest.updated_at = now_rfc3339();
    write_manifest(&manifest)?;
    emit_item(&app, &DownloadItem::from(&manifest));
    emit_queue(&app, &compute_queue_state(&manager));

    let result = async {
        let detail = plex::get_book_detail(&server_id, &rating_key).await?;
        let parts = plex::get_downloadable_parts(&server_id, &rating_key).await?;

        if parts.is_empty() {
            return Err(AppError::Message("no playable tracks to download".into()));
        }

        check_stop(&cancel, &pause).map_err(stop_to_err)?;

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
        if book_estimate > 0 {
            manifest.bytes_total = Some(book_estimate.max(manifest.bytes_downloaded));
        }
        manifest.updated_at = now_rfc3339();
        write_manifest(&manifest)?;
        emit_item(&app, &DownloadItem::from(&manifest));
        emit_queue(&app, &compute_queue_state(&manager));

        let client = download_http()?;

        if let Some(cover) = try_download_cover(&client, &detail, &dir, &cancel, &pause).await {
            manifest.cover_file = Some(cover.clone());
            let cover_path = dir.join(&cover);
            if let Err(e) = crate::covers::import_file(&server_id, &rating_key, &cover_path) {
                eprintln!("cover cache import failed: {e}");
            }
            write_manifest(&manifest)?;
        }

        let n_parts = parts.len();
        // Keep completed tracks from a previous run
        let mut track_metas: Vec<DownloadTrackMeta> = manifest
            .tracks
            .iter()
            .filter(|t| {
                let p = dir.join(&t.file_name);
                p.exists() && t.bytes > 0
            })
            .cloned()
            .collect();
        // Also accept track_NNN.ext from interrupted multi-part jobs
        for (i, part) in parts.iter().enumerate() {
            if track_metas.iter().any(|t| t.index == i as u32) {
                continue;
            }
            let ext = part.file_ext.trim_matches('.').to_ascii_lowercase();
            let ext = if ext.is_empty() {
                "bin".to_string()
            } else {
                ext
            };
            let candidate = dir.join(format!("track_{i:03}.{ext}"));
            if candidate.exists() {
                let bytes = fs::metadata(&candidate).map(|m| m.len()).unwrap_or(0);
                if bytes > 0 {
                    track_metas.push(DownloadTrackMeta {
                        index: i as u32,
                        rating_key: part.rating_key.clone(),
                        title: part.title.clone(),
                        duration_ms: part.duration_ms,
                        file_name: format!("track_{i:03}.{ext}"),
                        bytes,
                        container: Some(part.container.clone()),
                        playable_file_name: None,
                    });
                }
            }
        }
        track_metas.sort_by_key(|t| t.index);

        let mut last_emit_bytes = track_metas.iter().map(|t| t.bytes).sum::<u64>();
        let mut last_emit_at = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(2))
            .unwrap_or_else(std::time::Instant::now);

        for (i, part) in parts.iter().enumerate() {
            check_stop(&cancel, &pause).map_err(stop_to_err)?;

            // Skip parts already fully on disk
            if let Some(existing) = track_metas.iter().find(|t| t.index == i as u32) {
                if dir.join(&existing.file_name).exists() {
                    continue;
                }
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
                &pause,
                |n, content_length| {
                    let expected = content_length.or(size_est);
                    let (progress, total) =
                        whole_book_progress(completed_bytes, n, expected, &future_estimates);
                    manifest.progress = progress;
                    manifest.bytes_downloaded = completed_bytes.saturating_add(n);
                    manifest.bytes_total = Some(total);
                    manifest.tracks_done = i as u32;
                    manifest.updated_at = now_rfc3339();

                    // Throttle disk + UI: every ~2 MiB or 750ms (avoids UI freezes during play)
                    let delta = manifest.bytes_downloaded.saturating_sub(last_emit_bytes);
                    let due = last_emit_at.elapsed() >= std::time::Duration::from_millis(750);
                    if n == 0 || delta >= 2 * 1024 * 1024 || due {
                        last_emit_bytes = manifest.bytes_downloaded;
                        last_emit_at = std::time::Instant::now();
                        let _ = write_manifest(&manifest);
                        emit_item(&app, &DownloadItem::from(&manifest));
                        // Queue totals less often — scanning manifests is expensive
                        if delta >= 4 * 1024 * 1024 || n == 0 {
                            emit_queue(&app, &compute_queue_state(&manager));
                        }
                    }
                },
            )
            .await?;

            // Replace any stale meta for this index
            track_metas.retain(|t| t.index != track_index);
            track_metas.push(DownloadTrackMeta {
                index: track_index,
                rating_key: part.rating_key.clone(),
                title: part.title.clone(),
                duration_ms: part.duration_ms,
                file_name,
                bytes,
                container: Some(part.container.clone()),
                playable_file_name: None,
            });
            track_metas.sort_by_key(|t| t.index);

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
            manifest.tracks_done = track_metas.len() as u32;
            manifest.bytes_downloaded = done_bytes;
            manifest.bytes_total = Some(total_after.max(done_bytes));
            manifest.progress = if track_metas.len() >= n_parts {
                1.0
            } else {
                progress_after
            };
            manifest.updated_at = now_rfc3339();
            last_emit_bytes = done_bytes;
            write_manifest(&manifest)?;
            emit_item(&app, &DownloadItem::from(&manifest));
            emit_queue(&app, &compute_queue_state(&manager));
        }

        if track_metas.len() != n_parts {
            // Last chance: maybe files were renamed / healed on disk already
            if let Ok(Some(healed)) = heal_incomplete_manifest(&rating_key) {
                if healed.status == DownloadStatus::Complete {
                    let item = DownloadItem::from(&healed);
                    emit_item(&app, &item);
                    return Ok(item);
                }
            }
            return Err(AppError::Message(format!(
                "download incomplete: got {} of {} parts",
                track_metas.len(),
                n_parts
            )));
        }

        let stem = manifest.file_stem.clone().unwrap_or_else(|| {
            book_file_stem(
                &manifest.title,
                manifest.series.as_deref(),
                manifest.series_index,
                manifest.author.as_deref(),
            )
        });
        if let Err(e) = rename_tracks_to_stem(&dir, &stem, &mut track_metas) {
            eprintln!("rename download files failed: {e}");
            // Still complete with track_NNN names if files exist
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
        if let Err(e) = write_manifest(&manifest) {
            // Media is on disk — try once more after a heal pass
            eprintln!("complete manifest write failed: {e}");
            if let Ok(Some(healed)) = heal_incomplete_manifest(&rating_key) {
                if healed.status == DownloadStatus::Complete {
                    let item = DownloadItem::from(&healed);
                    emit_item(&app, &item);
                    return Ok(item);
                }
            }
            return Err(e);
        }

        // Build WebKit-safe MP3 sidecars for AAC/M4B (HTMLAudio cannot play them reliably).
        // Best-effort: download is still complete if ffmpeg is missing.
        let rk_play = rating_key.clone();
        match tauri::async_runtime::spawn_blocking(move || ensure_playable(&rk_play)).await {
            Ok(Ok(item)) => {
                emit_item(&app, &item);
                Ok(item)
            }
            Ok(Err(e)) => {
                eprintln!("ensure_playable after download: {e}");
                let item = DownloadItem::from(&manifest);
                emit_item(&app, &item);
                Ok(item)
            }
            Err(e) => {
                eprintln!("ensure_playable join: {e}");
                let item = DownloadItem::from(&manifest);
                emit_item(&app, &item);
                Ok(item)
            }
        }
    }
    .await;

    manager.unregister(&rating_key);

    match result {
        Ok(item) => {
            manager.with_inner(|inner| {
                DownloadManager::remove_from_order_locked(inner, &rating_key);
                DownloadManager::persist_queue_locked(inner);
            });
            emit_queue(&app, &compute_queue_state(&manager));
            Ok(item)
        }
        Err(e) => {
            let msg = e.to_string();
            let cancelled = msg.contains("cancelled");
            let paused = msg.contains("paused");

            // If the bytes made it to disk, prefer complete over a spurious error
            if !cancelled && !paused {
                if let Ok(Some(healed)) = heal_incomplete_manifest(&rating_key) {
                    if healed.status == DownloadStatus::Complete {
                        manager.with_inner(|inner| {
                            DownloadManager::remove_from_order_locked(inner, &rating_key);
                            DownloadManager::persist_queue_locked(inner);
                        });
                        let item = DownloadItem::from(&healed);
                        emit_item(&app, &item);
                        emit_queue(&app, &compute_queue_state(&manager));
                        return Ok(item);
                    }
                }
            }

            if let Ok(Some(mut m)) = read_manifest(&rating_key) {
                m.status = if cancelled {
                    DownloadStatus::Cancelled
                } else if paused {
                    DownloadStatus::Paused
                } else {
                    DownloadStatus::Error
                };
                m.error = if cancelled || paused {
                    None
                } else {
                    Some(msg.clone())
                };
                m.updated_at = now_rfc3339();
                let _ = write_manifest(&m);
                emit_item(&app, &DownloadItem::from(&m));
            }
            if cancelled {
                manager.with_inner(|inner| {
                    DownloadManager::remove_from_order_locked(inner, &rating_key);
                    DownloadManager::persist_queue_locked(inner);
                });
            }
            // On pause: keep in order; on error: keep in order for retry on resume
            emit_queue(&app, &compute_queue_state(&manager));
            if cancelled {
                Err(AppError::Message("download cancelled".into()))
            } else if paused {
                // Soft stop — not a failure for the caller
                get_download(&rating_key)?
                    .ok_or_else(|| AppError::Message("download paused".into()))
            } else {
                Err(e)
            }
        }
    }
}

/// Called when removing a book: cancel job + drop from queue + delete files.
pub fn remove_item(
    app: &AppHandle,
    manager: &Arc<DownloadManager>,
    rating_key: &str,
) -> AppResult<()> {
    manager.request_cancel_item(rating_key);
    manager.with_inner(|inner| {
        DownloadManager::remove_from_order_locked(inner, rating_key);
        DownloadManager::persist_queue_locked(inner);
    });
    // Give the worker a moment to notice cancel; files deleted regardless
    remove_download(rating_key)?;
    emit_queue(app, &compute_queue_state(manager));
    pump_queue(app.clone(), Arc::clone(manager));
    Ok(())
}

pub fn remove_all_items(app: &AppHandle, manager: &Arc<DownloadManager>) -> AppResult<u32> {
    let order = manager.order();
    for key in order {
        manager.request_cancel_item(&key);
    }
    manager.with_inner(|inner| {
        inner.order.clear();
        DownloadManager::persist_queue_locked(inner);
    });
    let n = remove_all_downloads()?;
    emit_queue(app, &compute_queue_state(manager));
    Ok(n)
}
