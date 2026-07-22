use super::models::{AuthStatus, PinAuthPoll, PinAuthStart};
use crate::error::{AppError, AppResult};
use crate::storage::config::AppConfig;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// In-memory pending PIN (real implementation will call plex.tv).
#[derive(Debug, Default)]
pub struct PendingPin {
    pub id: u64,
    pub code: String,
    pub created_at: u64,
    /// Simulated: becomes authorized after N polls or explicit dev bypass.
    pub poll_count: u32,
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

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Start PIN auth. Real flow: POST https://plex.tv/api/v2/pins
pub fn start_pin_auth(state: &PlexAuthState) -> AppResult<PinAuthStart> {
    let id = now_secs();
    // Placeholder code for UI wiring; replaced when HTTP client lands.
    let code = format!("RA{:04}", id % 10000);

    let mut guard = state
        .pending
        .lock()
        .map_err(|_| AppError::Message("auth state lock poisoned".into()))?;
    *guard = Some(PendingPin {
        id,
        code: code.clone(),
        created_at: now_secs(),
        poll_count: 0,
    });

    Ok(PinAuthStart {
        id,
        code: code.clone(),
        auth_url: format!("https://app.plex.tv/auth#?pinCode={code}&context[device][product]=RogueAudio"),
    })
}

/// Poll PIN. Real flow: GET https://plex.tv/api/v2/pins/{id}
///
/// Foundation stub never auto-authorizes. Use `dev_complete_pin_auth` in debug
/// builds, or replace with real plex.tv HTTP.
pub fn poll_pin_auth(state: &PlexAuthState) -> AppResult<PinAuthPoll> {
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

    let mut guard = state
        .pending
        .lock()
        .map_err(|_| AppError::Message("auth state lock poisoned".into()))?;

    if let Some(pending) = guard.as_mut() {
        pending.poll_count = pending.poll_count.saturating_add(1);
    }

    Ok(PinAuthPoll {
        authorized: false,
        status: AuthStatus {
            authenticated: false,
            username: None,
        },
    })
}

/// Development helper: simulate successful PIN authorization.
#[cfg(debug_assertions)]
pub fn dev_complete_pin_auth(state: &PlexAuthState, username: Option<String>) -> AppResult<AuthStatus> {
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
