//! Resolve Plex album/track metadata into playable stream URLs.

use super::models::{PlaybackInfo, PlaybackTrack};
use super::server::{connect, ServerSession};
use crate::error::{AppError, AppResult};
use serde_json::Value;

/// Build a playable playlist for a book (album) or single track ratingKey.
pub async fn get_playback(server_id: &str, rating_key: &str) -> AppResult<PlaybackInfo> {
    let session = connect(server_id).await?;
    let meta = fetch_metadata(&session, rating_key).await?;

    let meta_type = meta
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let track_nodes: Vec<Value> = match meta_type.as_str() {
        // Album / book container → child tracks
        "album" => {
            let children = fetch_children(&session, rating_key).await?;
            if children.is_empty() {
                // Some single-file books expose Media on the album itself
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
        // Direct track
        "track" | "episode" => vec![meta],
        // Fallback: try children, else self
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
        if let Some(t) = map_track(&node, &session, i as u32) {
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
    extract_part_key(meta).is_some()
}

fn extract_part_key(meta: &Value) -> Option<(String, Option<String>, Option<u64>)> {
    // Media[] → Part[] with key
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

fn map_track(meta: &Value, session: &ServerSession, index: u32) -> Option<PlaybackTrack> {
    let (part_key, container, part_duration) = extract_part_key(meta)?;
    let rating_key = meta
        .get("ratingKey")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let title = meta
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Track")
        .to_string();
    let duration_ms = part_duration.or_else(|| meta.get("duration").and_then(|v| v.as_u64()));
    let url = absolute_part_url(&session.base_uri, &part_key, &session.token);

    Some(PlaybackTrack {
        rating_key,
        title,
        index,
        duration_ms,
        url,
        container,
    })
}

fn absolute_part_url(base_uri: &str, part_key: &str, token: &str) -> String {
    let base = base_uri.trim_end_matches('/');
    if part_key.starts_with("http://") || part_key.starts_with("https://") {
        let sep = if part_key.contains('?') { '&' } else { '?' };
        if part_key.contains("X-Plex-Token=") {
            return part_key.to_string();
        }
        return format!("{part_key}{sep}X-Plex-Token={token}");
    }
    let path = if part_key.starts_with('/') {
        part_key.to_string()
    } else {
        format!("/{part_key}")
    };
    // download=1 can help some clients; streaming usually works without.
    format!("{base}{path}?X-Plex-Token={token}&download=0")
}
