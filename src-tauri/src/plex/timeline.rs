//! Plex Media Server timeline / resume offset helpers.
//!
//! Push: `GET /:/timeline/` so Plexamp and other clients can resume.
//! Pull: metadata `viewOffset` for the item.

use super::client::{client_identifier, pms_get_json, pms_get_text};
use super::server::connect;
use crate::error::{AppError, AppResult};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexProgress {
    pub rating_key: String,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    /// Unix seconds from Plex lastViewedAt when present.
    pub last_viewed_at: Option<i64>,
}

/// Fetch resume offset from PMS metadata (album/book rating key).
pub async fn fetch_view_offset(
    server_id: &str,
    rating_key: &str,
) -> AppResult<Option<PlexProgress>> {
    let session = connect(server_id).await?;
    let path = format!("/library/metadata/{rating_key}");
    let raw: Value = pms_get_json(&session.base_uri, &path, &session.token).await?;
    let meta = raw
        .pointer("/MediaContainer/Metadata/0")
        .cloned()
        .ok_or_else(|| AppError::Message(format!("metadata not found for {rating_key}")))?;

    let mut position_ms = meta.get("viewOffset").and_then(|v| v.as_u64()).unwrap_or(0);
    let duration_ms = meta.get("duration").and_then(|v| v.as_u64());
    let last_viewed_at = meta.get("lastViewedAt").and_then(|v| v.as_i64());

    // Multi-part albums: if album has no offset, try children and pick most recently viewed
    if position_ms == 0 {
        if let Ok(children) = fetch_children_offsets(&session.base_uri, &session.token, rating_key).await
        {
            if let Some(best) = children
                .into_iter()
                .filter(|c| c.position_ms > 0)
                .max_by_key(|c| c.last_viewed_at.unwrap_or(0))
            {
                // Use part offset as approximate book position when only one part has progress;
                // for multi-part continuous progress Plex usually stores per-track offset only.
                position_ms = best.position_ms;
            }
        }
    }

    if position_ms == 0 && last_viewed_at.is_none() {
        return Ok(None);
    }

    Ok(Some(PlexProgress {
        rating_key: rating_key.to_string(),
        position_ms,
        duration_ms,
        last_viewed_at,
    }))
}

async fn fetch_children_offsets(
    base_uri: &str,
    token: &str,
    rating_key: &str,
) -> AppResult<Vec<PlexProgress>> {
    let path = format!("/library/metadata/{rating_key}/children");
    let raw: Value = pms_get_json(base_uri, &path, token).await?;
    let items = raw
        .pointer("/MediaContainer/Metadata")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(items
        .into_iter()
        .filter_map(|m| {
            let rk = m.get("ratingKey")?.as_str()?.to_string();
            let position_ms = m.get("viewOffset").and_then(|v| v.as_u64()).unwrap_or(0);
            let duration_ms = m.get("duration").and_then(|v| v.as_u64());
            let last_viewed_at = m.get("lastViewedAt").and_then(|v| v.as_i64());
            Some(PlexProgress {
                rating_key: rk,
                position_ms,
                duration_ms,
                last_viewed_at,
            })
        })
        .collect())
}

/// Report playback state to PMS so other clients (Plexamp) can resume.
pub async fn post_timeline(
    server_id: &str,
    rating_key: &str,
    state: &str,
    time_ms: u64,
    duration_ms: Option<u64>,
    speed: f64,
) -> AppResult<()> {
    let session = connect(server_id).await?;
    let client_id = client_identifier()?;
    let key = format!("/library/metadata/{rating_key}");
    let duration = duration_ms.unwrap_or(0);
    let state = match state {
        "playing" | "paused" | "stopped" | "buffering" => state,
        _ => "paused",
    };

    // PMS expects GET /:/timeline/ with query params (widely used by official clients).
    let path = format!(
        "/:/timeline/?ratingKey={rk}&key={key}&state={state}&time={time}&duration={duration}&playbackTime={time}&continuing=1&hasMDE=1&playQueueItemID=0&X-Plex-Client-Identifier={cid}&X-Plex-Product=RogueAudio&X-Plex-Device=RogueAudio&X-Plex-Platform=Linux&X-Plex-Token={token}",
        rk = encode_component(rating_key),
        key = encode_component(&key),
        state = encode_component(state),
        time = time_ms,
        duration = duration,
        cid = encode_component(&client_id),
        token = encode_component(&session.token),
    );

    // Ignore body; 200/204 OK. Use get_text path which validates status.
    let _ = pms_get_text(&session.base_uri, &path, &session.token, false).await?;

    // Include rate when possible (best-effort second call not required)
    let _ = speed;
    Ok(())
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
