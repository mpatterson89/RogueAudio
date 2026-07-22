//! Resolve Plex album/track metadata into playable stream URLs.
//!
//! WebKit rejects many direct .m4b URLs because Plex serves them as
//! `application/octet-stream` (HTMLMediaElement error code 4). We therefore
//! use the Plex universal transcoder to deliver progressive `audio/mpeg`.

use super::client::client_identifier;
use super::models::{PlaybackInfo, PlaybackTrack};
use super::server::{connect, ServerSession};
use crate::error::{AppError, AppResult};
use serde_json::Value;

/// Build a playable playlist for a book (album) or single track ratingKey.
pub async fn get_playback(server_id: &str, rating_key: &str) -> AppResult<PlaybackInfo> {
    let session = connect(server_id).await?;
    let client_id = client_identifier()?;
    let meta = fetch_metadata(&session, rating_key).await?;

    let meta_type = meta
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let track_nodes: Vec<Value> = match meta_type.as_str() {
        "album" => {
            let children = fetch_children(&session, rating_key).await?;
            if children.is_empty() {
                if has_playable_media(&meta) {
                    vec![meta]
                } else {
                    return Err(AppError::Message(
                        "album has no playable tracks".into(),
                    ));
                }
            } else {
                children
            }
        }
        "track" | "episode" => vec![meta],
        _ => {
            let children = fetch_children(&session, rating_key).await.unwrap_or_default();
            if !children.is_empty() {
                children
            } else if has_playable_media(&meta) {
                vec![meta]
            } else {
                return Err(AppError::Message(format!(
                    "unsupported media type '{meta_type}' for playback"
                )));
            }
        }
    };

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

/// Returns (part_key, container, duration_ms)
fn extract_part_info(meta: &Value) -> Option<(String, Option<String>, Option<u64>)> {
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
            return Some((key, container, duration));
        }
    }
    None
}

fn map_track(
    meta: &Value,
    session: &ServerSession,
    client_id: &str,
    index: u32,
) -> Option<PlaybackTrack> {
    let (_part_key, _container, part_duration) = extract_part_info(meta)?;
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
    let duration_ms = part_duration.or_else(|| meta.get("duration").and_then(|v| v.as_u64()));

    // Transcode to progressive MP3 — correct Content-Type for WebKit / HTML5 audio.
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
