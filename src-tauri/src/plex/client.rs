//! HTTP client helpers for plex.tv and Plex Media Server.

use super::identity::{plex_headers, PLEX_CLIENT_IDENTIFIER};
use crate::error::{AppError, AppResult};
use crate::storage::config::AppConfig;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::de::DeserializeOwned;
use std::sync::OnceLock;

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
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client")
    })
}

/// Stable per-install client id (UUID). Falls back to product id only if storage fails mid-flight.
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

fn build_header_map(token: Option<&str>, client_id: &str) -> AppResult<HeaderMap> {
    let mut map = HeaderMap::new();
    for (k, v) in plex_headers(token) {
        let name = HeaderName::from_bytes(k.as_bytes())
            .map_err(|e| AppError::Message(format!("bad header name {k}: {e}")))?;
        let value = HeaderValue::from_str(&v)
            .map_err(|e| AppError::Message(format!("bad header value for {k}: {e}")))?;
        map.insert(name, value);
    }
    // Override client identifier with per-install id
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

    serde_json::from_str(&body).map_err(|e| {
        AppError::Message(format!(
            "plex.tv JSON parse failed for {path}: {e}; body starts: {}",
            body.chars().take(120).collect::<String>()
        ))
    })
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
