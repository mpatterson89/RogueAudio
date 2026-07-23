//! Resolve Plex album/track metadata into playable stream URLs.
//!
//! WebKit rejects many *remote* .m4b URLs because Plex serves them as
//! `application/octet-stream` (HTMLMediaElement error code 4). Online playback
//! therefore uses the Plex universal transcoder → progressive `audio/mpeg`.
//!
//! Offline downloads use the **original** part files (m4b/m4a/mp3/…) so size
//! and quality match the library.

use super::client::client_identifier;
use super::models::{PlaybackInfo, PlaybackTrack};
use super::server::{connect, ServerSession};
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One original media part suitable for offline download (not a transcoder URL).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadablePart {
    pub rating_key: String,
    pub title: String,
    pub index: u32,
    pub duration_ms: Option<u64>,
    /// Declared size from Plex Part.size when present.
    pub size_bytes: Option<u64>,
    /// Plex container label (m4b, mp3, …).
    pub container: String,
    /// File extension to use on disk (m4b, mp3, …).
    pub file_ext: String,
    /// Fully-qualified original-file URL including token.
    pub url: String,
}

/// Build a playable playlist for a book (album) or single track ratingKey.
pub async fn get_playback(server_id: &str, rating_key: &str) -> AppResult<PlaybackInfo> {
    let session = connect(server_id).await?;
    let client_id = client_identifier()?;
    let meta = fetch_metadata(&session, rating_key).await?;
    let track_nodes = resolve_track_nodes(&session, rating_key, meta).await?;

    let mut tracks: Vec<PlaybackTrack> = Vec::new();
    for (i, node) in track_nodes.into_iter().enumerate() {
        if let Some(t) = map_track(&node, &session, &client_id, i as u32) {
            tracks.push(t);
        }
    }

    if tracks.is_empty() {
        return Err(AppError::Message(
            "no playable audio parts found for this item".into(),
        ));
    }

    let total_duration_ms = {
        let sum: u64 = tracks.iter().filter_map(|t| t.duration_ms).sum();
        if sum > 0 {
            Some(sum)
        } else {
            None
        }
    };

    Ok(PlaybackInfo {
        book_rating_key: rating_key.to_string(),
        tracks,
        total_duration_ms,
    })
}

async fn fetch_metadata(session: &ServerSession, rating_key: &str) -> AppResult<Value> {
    let path = format!("/library/metadata/{rating_key}");
    let raw: Value = super::client::pms_get_json(&session.base_uri, &path, &session.token).await?;
    raw.pointer("/MediaContainer/Metadata/0")
        .cloned()
        .ok_or_else(|| AppError::Message(format!("metadata not found for {rating_key}")))
}

async fn fetch_children(session: &ServerSession, rating_key: &str) -> AppResult<Vec<Value>> {
    let path = format!("/library/metadata/{rating_key}/children");
    let raw: Value = super::client::pms_get_json(&session.base_uri, &path, &session.token).await?;
    Ok(raw
        .pointer("/MediaContainer/Metadata")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default())
}

fn has_playable_media(meta: &Value) -> bool {
    extract_part_info(meta).is_some()
}

struct PartInfo {
    /// e.g. `/library/parts/123/0/file.m4b`
    key: String,
    container: Option<String>,
    duration_ms: Option<u64>,
    size_bytes: Option<u64>,
    /// Original file name from Plex when present (`Something.m4b`).
    file: Option<String>,
}

fn extract_part_info(meta: &Value) -> Option<PartInfo> {
    let media = meta.get("Media")?.as_array()?;
    for m in media {
        let parts = m.get("Part")?.as_array()?;
        for p in parts {
            let key = p.get("key")?.as_str()?.to_string();
            let container = p
                .get("container")
                .and_then(|v| v.as_str())
                .or_else(|| m.get("container").and_then(|v| v.as_str()))
                .map(|s| s.to_string());
            let duration = p
                .get("duration")
                .and_then(|v| v.as_u64())
                .or_else(|| m.get("duration").and_then(|v| v.as_u64()))
                .or_else(|| meta.get("duration").and_then(|v| v.as_u64()));
            let size_bytes = p.get("size").and_then(|v| v.as_u64());
            let file = p
                .get("file")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());
            return Some(PartInfo {
                key,
                container,
                duration_ms: duration,
                size_bytes,
                file,
            });
        }
    }
    None
}

fn extension_for_part(part: &PartInfo) -> String {
    // Prefer extension from the server path / file field (most accurate for m4b).
    if let Some(file) = part.file.as_deref() {
        if let Some(ext) = file.rsplit('.').next() {
            let e = ext.to_ascii_lowercase();
            if matches!(
                e.as_str(),
                "m4b" | "m4a" | "mp3" | "flac" | "ogg" | "opus" | "aac" | "wav" | "mp4" | "aiff" | "wma"
            ) {
                return e;
            }
        }
    }
    if let Some(ext) = part.key.rsplit('.').next() {
        let e = ext.to_ascii_lowercase();
        // key may end with `file.m4b` or just a number — only trust known audio ext
        if matches!(
            e.as_str(),
            "m4b" | "m4a" | "mp3" | "flac" | "ogg" | "opus" | "aac" | "wav" | "mp4"
        ) {
            return e;
        }
    }
    match part
        .container
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "m4b" => "m4b".into(),
        "mp3" | "mpeg" => "mp3".into(),
        "flac" => "flac".into(),
        "aac" | "m4a" => "m4a".into(),
        "mp4" => "mp4".into(),
        "ogg" | "vorbis" => "ogg".into(),
        "opus" => "opus".into(),
        other if !other.is_empty() => other.into(),
        _ => "bin".into(),
    }
}

/// Direct original-file download URL (no transcoder).
fn original_part_url(base_uri: &str, part_key: &str, token: &str) -> String {
    let base = base_uri.trim_end_matches('/');
    let key = if part_key.starts_with('/') {
        part_key.to_string()
    } else {
        format!("/{part_key}")
    };
    // download=1 asks PMS for the raw file bytes with a download disposition.
    format!(
        "{base}{key}?download=1&X-Plex-Token={}",
        encode_component(token)
    )
}

async fn resolve_track_nodes(
    session: &ServerSession,
    rating_key: &str,
    meta: Value,
) -> AppResult<Vec<Value>> {
    let meta_type = meta
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match meta_type.as_str() {
        "album" => {
            let children = fetch_children(session, rating_key).await?;
            if children.is_empty() {
                if has_playable_media(&meta) {
                    Ok(vec![meta])
                } else {
                    Err(AppError::Message("album has no playable tracks".into()))
                }
            } else {
                Ok(children)
            }
        }
        "track" | "episode" => Ok(vec![meta]),
        _ => {
            let children = fetch_children(session, rating_key).await.unwrap_or_default();
            if !children.is_empty() {
                Ok(children)
            } else if has_playable_media(&meta) {
                Ok(vec![meta])
            } else {
                Err(AppError::Message(format!(
                    "unsupported media type '{meta_type}' for playback"
                )))
            }
        }
    }
}

fn map_track(
    meta: &Value,
    session: &ServerSession,
    client_id: &str,
    index: u32,
) -> Option<PlaybackTrack> {
    let part = extract_part_info(meta)?;
    let rating_key = meta
        .get("ratingKey")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if rating_key.is_empty() {
        return None;
    }
    let title = meta
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Track")
        .to_string();
    let duration_ms = part
        .duration_ms
        .or_else(|| meta.get("duration").and_then(|v| v.as_u64()));

    // Online: transcode to progressive MP3 — correct Content-Type for WebKit.
    let url = transcode_mp3_url(
        &session.base_uri,
        &rating_key,
        &session.token,
        client_id,
    );

    Some(PlaybackTrack {
        rating_key,
        title,
        index,
        duration_ms,
        url,
        container: Some("mp3".into()),
    })
}

fn map_downloadable_part(
    meta: &Value,
    session: &ServerSession,
    index: u32,
) -> Option<DownloadablePart> {
    let part = extract_part_info(meta)?;
    let rating_key = meta
        .get("ratingKey")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if rating_key.is_empty() {
        return None;
    }
    let title = meta
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Track")
        .to_string();
    let duration_ms = part
        .duration_ms
        .or_else(|| meta.get("duration").and_then(|v| v.as_u64()));
    let file_ext = extension_for_part(&part);
    let container = part
        .container
        .clone()
        .unwrap_or_else(|| file_ext.clone());
    let url = original_part_url(&session.base_uri, &part.key, &session.token);

    Some(DownloadablePart {
        rating_key,
        title,
        index,
        duration_ms,
        size_bytes: part.size_bytes,
        container,
        file_ext,
        url,
    })
}

/// Original library files for offline download (m4b/mp3/… — not transcoded).
pub async fn get_downloadable_parts(
    server_id: &str,
    rating_key: &str,
) -> AppResult<Vec<DownloadablePart>> {
    let session = connect(server_id).await?;
    let meta = fetch_metadata(&session, rating_key).await?;
    let track_nodes = resolve_track_nodes(&session, rating_key, meta).await?;

    let mut parts: Vec<DownloadablePart> = Vec::new();
    for (i, node) in track_nodes.into_iter().enumerate() {
        if let Some(p) = map_downloadable_part(&node, &session, i as u32) {
            parts.push(p);
        }
    }

    if parts.is_empty() {
        return Err(AppError::Message(
            "no original media parts found for download".into(),
        ));
    }
    Ok(parts)
}

/// Universal transcoder → progressive MP3 (verified against PMS).
fn transcode_mp3_url(base_uri: &str, track_rating_key: &str, token: &str, client_id: &str) -> String {
    let base = base_uri.trim_end_matches('/');
    let session = uuid::Uuid::new_v4();
    let path = format!("/library/metadata/{track_rating_key}");

    // Keep param order stable; encode values.
    let q = [
        ("hasMDE", "1".to_string()),
        ("path", path),
        ("mediaIndex", "0".to_string()),
        ("partIndex", "0".to_string()),
        ("protocol", "http".to_string()),
        ("fastSeek", "1".to_string()),
        ("directPlay", "0".to_string()),
        ("directStream", "0".to_string()),
        ("location", "lan".to_string()),
        ("maxAudioBitrate", "320".to_string()),
        ("session", session.to_string()),
        ("copyType", "transcode".to_string()),
        ("audioCodec", "mp3".to_string()),
        ("X-Plex-Client-Identifier", client_id.to_string()),
        ("X-Plex-Product", "RogueAudio".to_string()),
        ("X-Plex-Platform", "Chrome".to_string()),
        ("X-Plex-Platform-Version", "120.0".to_string()),
        ("X-Plex-Device", "RogueAudio".to_string()),
        ("X-Plex-Device-Name", "RogueAudio".to_string()),
        ("X-Plex-Token", token.to_string()),
    ]
    .into_iter()
    .map(|(k, v)| format!("{}={}", k, encode_component(&v)))
    .collect::<Vec<_>>()
    .join("&");

    format!("{base}/music/:/transcode/universal/start.mp3?{q}")
}

fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            b'/' => out.push_str("%2F"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
