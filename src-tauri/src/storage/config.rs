use super::config_path;
use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// Plex auth token (null when logged out).
    pub plex_token: Option<String>,
    /// Plex account username when known.
    pub plex_username: Option<String>,
    /// Stable per-install Plex client identifier (UUID).
    pub plex_client_id: Option<String>,
    /// Preferred server machine identifier.
    pub preferred_server_id: Option<String>,
    /// Playback speed last used (UI may also keep its own copy).
    pub last_playback_rate: Option<f64>,
}

impl AppConfig {
    pub fn load() -> AppResult<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)?;
        let cfg = serde_json::from_str(&raw).unwrap_or_default();
        Ok(cfg)
    }

    pub fn save(&self) -> AppResult<()> {
        let path = config_path()?;
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| crate::error::AppError::Message(e.to_string()))?;
        fs::write(path, raw)?;
        Ok(())
    }
}
