//! HTTP client helpers for plex.tv and Plex Media Server.

use super::identity::{plex_headers, PLEX_CLIENT_IDENTIFIER};
use crate::error::{AppError, AppResult};
use crate::storage::config::AppConfig;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::de::DeserializeOwned;
use std::sync::OnceLock;
use std::time::Duration;

const PLEX_TV: &str = "https://plex.tv";

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(format!(
                "RogueAudio/{} ({})",
                env!("CARGO_PKG_VERSION"),
                PLEX_CLIENT_IDENTIFIER
            ))
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("failed to build HTTP client")
    })
}

/// Shorter-timeout client for probing PMS connections.
fn http_probe() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(format!(
                "RogueAudio/{} ({})",
                env!("CARGO_PKG_VERSION"),
                PLEX_CLIENT_IDENTIFIER
            ))
            .timeout(Duration::from_secs(4))
            .connect_timeout(Duration::from_secs(2))
            .build()
            .expect("failed to build probe HTTP client")
    })
}

/// Stable per-install client id (UUID).
pub fn client_identifier() -> AppResult<String> {
    let mut cfg = AppConfig::load()?;
    if let Some(id) = cfg.plex_client_id.clone() {
        if !id.is_empty() {
            return Ok(id);
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    cfg.plex_client_id = Some(id.clone());
    cfg.save()?;
    Ok(id)
}

pub fn account_token() -> AppResult<String> {
    AppConfig::load()?
        .plex_token
        .filter(|t| !t.is_empty())
        .ok_or(AppError::NotAuthenticated)
}

fn build_header_map(token: Option<&str>, client_id: &str) -> AppResult<HeaderMap> {
    let mut map = HeaderMap::new();
    for (k, v) in plex_headers(token) {
        let name = HeaderName::from_bytes(k.as_bytes())
            .map_err(|e| AppError::Message(format!("bad header name {k}: {e}")))?;
        let value = HeaderValue::from_str(&v)
            .map_err(|e| AppError::Message(format!("bad header value for {k}: {e}")))?;
        map.insert(name, value);
    }
    map.insert(
        HeaderName::from_static("x-plex-client-identifier"),
        HeaderValue::from_str(client_id)
            .map_err(|e| AppError::Message(format!("bad client id: {e}")))?,
    );
    Ok(map)
}

pub async fn plex_tv_get_json<T: DeserializeOwned>(
    path: &str,
    token: Option<&str>,
) -> AppResult<T> {
    let body = plex_tv_get_text(path, token).await?;
    serde_json::from_str(&body).map_err(|e| {
        AppError::Message(format!(
            "plex.tv JSON parse failed for {path}: {e}; body starts: {}",
            body.chars().take(120).collect::<String>()
        ))
    })
}

pub async fn plex_tv_get_text(path: &str, token: Option<&str>) -> AppResult<String> {
    let client_id = client_identifier()?;
    let headers = build_header_map(token, &client_id)?;
    let url = format!("{PLEX_TV}{path}");
    let res = http()
        .get(&url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| AppError::Message(format!("plex.tv request failed: {e}")))?;

    let status = res.status();
    let body = res
        .text()
        .await
        .map_err(|e| AppError::Message(format!("plex.tv read body failed: {e}")))?;

    if !status.is_success() {
        return Err(AppError::Message(format!(
            "plex.tv {path} returned {status}: {}",
            body.chars().take(200).collect::<String>()
        )));
    }
    Ok(body)
}

pub async fn plex_tv_post_json<T: DeserializeOwned>(
    path: &str,
    token: Option<&str>,
) -> AppResult<T> {
    let client_id = client_identifier()?;
    let headers = build_header_map(token, &client_id)?;
    let url = format!("{PLEX_TV}{path}");
    let res = http()
        .post(&url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| AppError::Message(format!("plex.tv request failed: {e}")))?;

    let status = res.status();
    let body = res
        .text()
        .await
        .map_err(|e| AppError::Message(format!("plex.tv read body failed: {e}")))?;

    if !status.is_success() {
        return Err(AppError::Message(format!(
            "plex.tv {path} returned {status}: {}",
            body.chars().take(200).collect::<String>()
        )));
    }

    serde_json::from_str(&body).map_err(|e| {
        AppError::Message(format!(
            "plex.tv JSON parse failed for {path}: {e}; body starts: {}",
            body.chars().take(120).collect::<String>()
        ))
    })
}

/// GET JSON from a Plex Media Server base URI (e.g. https://host:32400).
pub async fn pms_get_json<T: DeserializeOwned>(
    base_uri: &str,
    path_and_query: &str,
    token: &str,
) -> AppResult<T> {
    let body = pms_get_text(base_uri, path_and_query, token, false).await?;
    serde_json::from_str(&body).map_err(|e| {
        AppError::Message(format!(
            "PMS JSON parse failed for {path_and_query}: {e}; body starts: {}",
            body.chars().take(160).collect::<String>()
        ))
    })
}

pub async fn pms_get_text(
    base_uri: &str,
    path_and_query: &str,
    token: &str,
    probe: bool,
) -> AppResult<String> {
    let client_id = client_identifier()?;
    let headers = build_header_map(Some(token), &client_id)?;
    let base = base_uri.trim_end_matches('/');
    let path = if path_and_query.starts_with('/') {
        path_and_query.to_string()
    } else {
        format!("/{path_and_query}")
    };
    let url = format!("{base}{path}");

    let client = if probe { http_probe() } else { http() };
    let res = client
        .get(&url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| AppError::Message(format!("PMS request failed ({url}): {e}")))?;

    let status = res.status();
    let body = res
        .text()
        .await
        .map_err(|e| AppError::Message(format!("PMS read body failed: {e}")))?;

    if !status.is_success() {
        return Err(AppError::Message(format!(
            "PMS {path} returned {status}: {}",
            body.chars().take(200).collect::<String>()
        )));
    }
    Ok(body)
}

/// Returns true if identity endpoint responds OK.
pub async fn pms_reachable(base_uri: &str, token: &str) -> bool {
    pms_get_text(base_uri, "/identity", token, true).await.is_ok()
}
