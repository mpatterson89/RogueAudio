//! Plex Media Server discovery, library sections, and album (book) listing.

use super::client::{account_token, plex_tv_get_json, pms_get_json, pms_reachable};
use super::models::{AudiobookSummary, PlexConnection, PlexLibrary, PlexServer};
use crate::error::{AppError, AppResult};
use crate::storage::config::AppConfig;
use serde::Deserialize;

// --- plex.tv resources ---

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceDto {
    name: Option<String>,
    product: Option<String>,
    #[serde(default)]
    provides: Option<String>,
    client_identifier: Option<String>,
    #[serde(default)]
    public_address: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    owned: Option<bool>,
    #[serde(default)]
    connections: Vec<ConnectionDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionDto {
    uri: Option<String>,
    #[serde(default)]
    local: Option<bool>,
    #[serde(default)]
    relay: Option<bool>,
}

struct InternalServer {
    server: PlexServer,
    /// Token for this resource (shared servers often have a dedicated token).
    access_token: String,
}

async fn load_internal_servers() -> AppResult<Vec<InternalServer>> {
    let token = account_token()?;
    let resources: Vec<ResourceDto> = plex_tv_get_json(
        "/api/v2/resources?includeHttps=1&includeRelay=1",
        Some(&token),
    )
    .await?;

    let mut servers: Vec<InternalServer> = resources
        .into_iter()
        .filter(|r| {
            r.provides
                .as_deref()
                .unwrap_or("")
                .split(',')
                .any(|p| p.trim() == "server")
        })
        .filter_map(|r| {
            let id = r.client_identifier.filter(|s| !s.is_empty())?;
            let access_token = r
                .access_token
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| token.clone());

            let connections = r
                .connections
                .into_iter()
                .filter_map(|c| {
                    let uri = c.uri.filter(|u| !u.is_empty())?;
                    Some(PlexConnection {
                        uri,
                        local: c.local.unwrap_or(false),
                        relay: c.relay.unwrap_or(false),
                    })
                })
                .collect::<Vec<_>>();

            if connections.is_empty() {
                return None;
            }

            Some(InternalServer {
                server: PlexServer {
                    id,
                    name: r.name.unwrap_or_else(|| "Plex Server".into()),
                    product: r.product,
                    provides: r.provides,
                    public_address: r.public_address,
                    owned: r.owned.unwrap_or(false),
                    connections,
                },
                access_token,
            })
        })
        .collect();

    servers.sort_by(|a, b| {
        b.server
            .owned
            .cmp(&a.server.owned)
            .then(a.server.name.cmp(&b.server.name))
    });

    if let Ok(cfg) = AppConfig::load() {
        if let Some(pref) = cfg.preferred_server_id {
            if let Some(idx) = servers.iter().position(|s| s.server.id == pref) {
                let preferred = servers.remove(idx);
                servers.insert(0, preferred);
            }
        }
    }

    Ok(servers)
}

/// Discover Plex Media Servers for the signed-in account.
pub async fn list_servers() -> AppResult<Vec<PlexServer>> {
    Ok(load_internal_servers()
        .await?
        .into_iter()
        .map(|s| s.server)
        .collect())
}

/// Resolved, reachable PMS endpoint for a server id.
#[derive(Debug, Clone)]
pub struct ServerSession {
    pub base_uri: String,
    pub token: String,
}

/// Find a working connection for `server_id` (local non-relay first).
pub async fn connect(server_id: &str) -> AppResult<ServerSession> {
    let servers = load_internal_servers().await?;
    let internal = servers
        .into_iter()
        .find(|s| s.server.id == server_id)
        .ok_or_else(|| AppError::Message(format!("server not found: {server_id}")))?;

    let mut conns = internal.server.connections;
    conns.sort_by_key(|c| match (c.local, c.relay) {
        (true, false) => 0,
        (false, false) => 1,
        (true, true) => 2,
        (false, true) => 3,
    });

    let token = internal.access_token;
    let mut last_err = AppError::Message("no connections to try".into());

    for c in conns {
        if pms_reachable(&c.uri, &token).await {
            if let Ok(mut cfg) = AppConfig::load() {
                cfg.preferred_server_id = Some(server_id.to_string());
                let _ = cfg.save();
            }
            return Ok(ServerSession {
                base_uri: c.uri,
                token,
            });
        }
        last_err = AppError::Message(format!("unreachable: {}", c.uri));
    }

    Err(AppError::Message(format!(
        "could not reach server '{}': {last_err}",
        internal.server.name
    )))
}

// --- library sections ---

pub async fn list_libraries(server_id: &str) -> AppResult<Vec<PlexLibrary>> {
    let session = connect(server_id).await?;
    let raw: serde_json::Value =
        pms_get_json(&session.base_uri, "/library/sections", &session.token).await?;

    let dirs = raw
        .pointer("/MediaContainer/Directory")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut libraries: Vec<PlexLibrary> = dirs
        .into_iter()
        .filter_map(|d| {
            let key = d.get("key")?.as_str()?.to_string();
            let title = d
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Library")
                .to_string();
            let library_type = d
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let agent = d
                .get("agent")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Some(PlexLibrary {
                key,
                title,
                library_type,
                agent,
            })
        })
        .collect();

    libraries.retain(|l| l.is_music_section());
    libraries.sort_by_key(|l| (!l.looks_like_audiobooks(), l.title.to_ascii_lowercase()));
    Ok(libraries)
}

// --- albums as books ---

/// Music album type id in Plex.
const TYPE_ALBUM: &str = "9";

pub async fn list_books(
    server_id: &str,
    library_key: &str,
    query: Option<&str>,
) -> AppResult<Vec<AudiobookSummary>> {
    let session = connect(server_id).await?;
    let q = query.map(str::trim).filter(|s| !s.is_empty());

    let path = if let Some(q) = q {
        format!(
            "/library/sections/{library_key}/search?type={TYPE_ALBUM}&query={}",
            urlencoding_form(q)
        )
    } else {
        format!("/library/sections/{library_key}/all?type={TYPE_ALBUM}&sort=titleSort")
    };

    let raw: serde_json::Value =
        pms_get_json(&session.base_uri, &path, &session.token).await?;

    let items = raw
        .pointer("/MediaContainer/Metadata")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let books = items
        .into_iter()
        .filter_map(|m| map_album_metadata(&m, &session, library_key))
        .collect();

    Ok(books)
}

fn map_album_metadata(
    m: &serde_json::Value,
    session: &ServerSession,
    library_key: &str,
) -> Option<AudiobookSummary> {
    let rating_key = m.get("ratingKey")?.as_str()?.to_string();
    let title = m
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string();

    let author = m
        .get("parentTitle")
        .and_then(|v| v.as_str())
        .or_else(|| m.get("originalTitle").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    let year = m.get("year").and_then(|v| v.as_i64()).map(|y| y as i32);

    let duration_ms = m.get("duration").and_then(|v| v.as_u64()).or_else(|| {
        m.pointer("/Media/0/duration").and_then(|v| v.as_u64())
    });

    let thumb = m
        .get("thumb")
        .and_then(|v| v.as_str())
        .map(|path| absolute_media_url(&session.base_uri, path, &session.token));

    Some(AudiobookSummary {
        rating_key,
        title,
        author,
        thumb,
        year,
        duration_ms,
        library_key: Some(library_key.into()),
    })
}

fn absolute_media_url(base_uri: &str, path: &str, token: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        if path.contains("X-Plex-Token=") {
            return path.to_string();
        }
        let sep = if path.contains('?') { '&' } else { '?' };
        return format!("{path}{sep}X-Plex-Token={token}");
    }
    let base = base_uri.trim_end_matches('/');
    let p = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    let sep = if p.contains('?') { '&' } else { '?' };
    format!("{base}{p}{sep}X-Plex-Token={token}")
}

fn urlencoding_form(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
