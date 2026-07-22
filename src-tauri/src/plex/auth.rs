use super::client::{client_identifier, plex_tv_get_json, plex_tv_post_json};
use super::identity::PLEX_PRODUCT;
use super::models::{AuthStatus, PinAuthPoll, PinAuthStart};
use crate::error::{AppError, AppResult};
use crate::storage::config::AppConfig;
use serde::Deserialize;
use std::sync::Mutex;

/// In-memory pending PIN from plex.tv.
#[derive(Debug, Clone)]
pub struct PendingPin {
    pub id: u64,
    pub code: String,
    pub client_id: String,
}

pub struct PlexAuthState {
    pub pending: Mutex<Option<PendingPin>>,
}

impl Default for PlexAuthState {
    fn default() -> Self {
        Self {
            pending: Mutex::new(None),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinResponse {
    id: u64,
    code: String,
    #[serde(default)]
    auth_token: Option<String>,
    #[serde(default)]
    client_identifier: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlexUser {
    /// Preferred display field when present.
    #[serde(default)]
    friendly_name: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

fn urlencoding_minimal(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

fn auth_url(client_id: &str, code: &str) -> String {
    // Forward-auth URL used by official / third-party clients.
    // User can also enter `code` manually at https://plex.tv/link
    format!(
        "https://app.plex.tv/auth#?clientID={client_id}&code={code}&context%5Bdevice%5D%5Bproduct%5D={product}",
        client_id = urlencoding_minimal(client_id),
        code = urlencoding_minimal(code),
        product = urlencoding_minimal(PLEX_PRODUCT),
    )
}

/// Start PIN auth via POST https://plex.tv/api/v2/pins
///
/// Uses a short 4-character code so users can type it at https://plex.tv/link.
/// (`strong=true` returns a long code for the browser auth# flow only.)
pub async fn start_pin_auth(state: &PlexAuthState) -> AppResult<PinAuthStart> {
    let client_id = client_identifier()?;

    let pin: PinResponse = plex_tv_post_json("/api/v2/pins", None).await?;

    let mut guard = state
        .pending
        .lock()
        .map_err(|_| AppError::Message("auth state lock poisoned".into()))?;
    *guard = Some(PendingPin {
        id: pin.id,
        code: pin.code.clone(),
        client_id: pin
            .client_identifier
            .clone()
            .unwrap_or_else(|| client_id.clone()),
    });

    let cid = pin.client_identifier.as_deref().unwrap_or(&client_id);
    Ok(PinAuthStart {
        id: pin.id,
        code: pin.code.clone(),
        auth_url: auth_url(cid, &pin.code),
    })
}

/// Poll PIN via GET https://plex.tv/api/v2/pins/{id}
pub async fn poll_pin_auth(state: &PlexAuthState) -> AppResult<PinAuthPoll> {
    let cfg = AppConfig::load()?;
    if cfg.plex_token.is_some() {
        return Ok(PinAuthPoll {
            authorized: true,
            status: AuthStatus {
                authenticated: true,
                username: cfg.plex_username,
            },
        });
    }

    let pending = {
        let guard = state
            .pending
            .lock()
            .map_err(|_| AppError::Message("auth state lock poisoned".into()))?;
        guard.clone()
    };

    let Some(pending) = pending else {
        return Ok(PinAuthPoll {
            authorized: false,
            status: AuthStatus {
                authenticated: false,
                username: None,
            },
        });
    };

    let path = format!("/api/v2/pins/{}", pending.id);
    let pin: PinResponse = plex_tv_get_json(&path, None).await?;

    if let Some(token) = pin.auth_token.filter(|t| !t.is_empty()) {
        let username = fetch_username(&token).await.ok();
        let mut cfg = AppConfig::load()?;
        cfg.plex_token = Some(token);
        cfg.plex_username = username.clone();
        cfg.save()?;

        let mut guard = state
            .pending
            .lock()
            .map_err(|_| AppError::Message("auth state lock poisoned".into()))?;
        *guard = None;

        return Ok(PinAuthPoll {
            authorized: true,
            status: AuthStatus {
                authenticated: true,
                username,
            },
        });
    }

    Ok(PinAuthPoll {
        authorized: false,
        status: AuthStatus {
            authenticated: false,
            username: None,
        },
    })
}

async fn fetch_username(token: &str) -> AppResult<String> {
    let user: PlexUser = plex_tv_get_json("/api/v2/user", Some(token)).await?;
    Ok(user
        .friendly_name
        .or(user.username)
        .or(user.title)
        .or(user.email)
        .unwrap_or_else(|| "Plex user".into()))
}

/// Development helper: simulate successful PIN authorization (no plex.tv).
#[cfg(debug_assertions)]
pub fn dev_complete_pin_auth(
    state: &PlexAuthState,
    username: Option<String>,
) -> AppResult<AuthStatus> {
    let mut guard = state
        .pending
        .lock()
        .map_err(|_| AppError::Message("auth state lock poisoned".into()))?;
    guard.take();

    let mut cfg = AppConfig::load()?;
    cfg.plex_token = Some("dev-token-not-for-production".into());
    cfg.plex_username = Some(username.unwrap_or_else(|| "Dev Listener".into()));
    cfg.save()?;

    Ok(AuthStatus {
        authenticated: true,
        username: cfg.plex_username,
    })
}

pub fn logout() -> AppResult<()> {
    let mut cfg = AppConfig::load()?;
    cfg.plex_token = None;
    cfg.plex_username = None;
    cfg.save()?;
    Ok(())
}

pub fn auth_status() -> AppResult<AuthStatus> {
    let cfg = AppConfig::load()?;
    Ok(AuthStatus {
        authenticated: cfg.plex_token.is_some(),
        username: cfg.plex_username,
    })
}
