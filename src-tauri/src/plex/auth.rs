use super::client::{client_identifier, plex_tv_get_json, plex_tv_post_json};
use super::identity::{
    PLEX_DEVICE, PLEX_PLATFORM, PLEX_PLATFORM_VERSION, PLEX_PRODUCT, PLEX_VERSION,
};
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
    #[serde(default)]
    friendly_name: Option<String>,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

fn urlencoding_form(s: &str) -> String {
    // application/x-www-form-urlencoded style (spaces as %20)
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Build the Plex OAuth URL.
///
/// Critical: path must be `/auth/#!?` (hashbang), not `/auth#?`.
/// Matches python-plexapi `MyPlexPinLogin.oauthUrl()`.
fn oauth_url(client_id: &str, code: &str) -> String {
    let mut q = String::new();
    let push = |q: &mut String, key: &str, value: &str| {
        if !q.is_empty() {
            q.push('&');
        }
        q.push_str(&urlencoding_form(key));
        q.push('=');
        q.push_str(&urlencoding_form(value));
    };

    push(&mut q, "clientID", client_id);
    push(&mut q, "code", code);
    push(&mut q, "context[device][product]", PLEX_PRODUCT);
    push(&mut q, "context[device][version]", PLEX_VERSION);
    push(&mut q, "context[device][platform]", PLEX_PLATFORM);
    push(
        &mut q,
        "context[device][platformVersion]",
        PLEX_PLATFORM_VERSION,
    );
    push(&mut q, "context[device][device]", PLEX_DEVICE);
    push(&mut q, "context[device][deviceName]", PLEX_DEVICE);

    format!("https://app.plex.tv/auth/#!?{q}")
}

/// Start OAuth-style PIN auth: POST https://plex.tv/api/v2/pins?strong=true
///
/// Strong codes are long secrets used only with the app.plex.tv/auth browser flow —
/// not typed at plex.tv/link.
pub async fn start_pin_auth(state: &PlexAuthState) -> AppResult<PinAuthStart> {
    let client_id = client_identifier()?;

    let pin: PinResponse = plex_tv_post_json("/api/v2/pins?strong=true", None).await?;

    let cid = pin
        .client_identifier
        .clone()
        .unwrap_or_else(|| client_id.clone());

    let mut guard = state
        .pending
        .lock()
        .map_err(|_| AppError::Message("auth state lock poisoned".into()))?;
    *guard = Some(PendingPin {
        id: pin.id,
        code: pin.code.clone(),
        client_id: cid.clone(),
    });

    Ok(PinAuthStart {
        id: pin.id,
        code: pin.code.clone(),
        auth_url: oauth_url(&cid, &pin.code),
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
