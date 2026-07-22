//! Plex client identity used in auth and API headers.
//! Keep product + client identifier stable so Plex devices list stays consistent.

/// Display / product name shown in Plex "authorized devices".
pub const PLEX_PRODUCT: &str = "RogueAudio";

/// Reverse-DNS style client identifier (matches Tauri app identifier).
pub const PLEX_CLIENT_IDENTIFIER: &str = "app.rogueaudio";

/// Platform string for X-Plex-Platform (overridden per-OS later if needed).
#[cfg(target_os = "linux")]
pub const PLEX_PLATFORM: &str = "Linux";

#[cfg(target_os = "windows")]
pub const PLEX_PLATFORM: &str = "Windows";

#[cfg(target_os = "macos")]
pub const PLEX_PLATFORM: &str = "macOS";

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub const PLEX_PLATFORM: &str = "Unknown";

/// Device name hint for Plex.
pub const PLEX_DEVICE: &str = "RogueAudio";

/// Version sent as X-Plex-Version (keep in sync with app version when packaging).
pub const PLEX_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Standard X-Plex-* headers for plex.tv and PMS requests.
pub fn plex_headers(token: Option<&str>) -> Vec<(String, String)> {
    let mut headers = vec![
        ("X-Plex-Product".into(), PLEX_PRODUCT.into()),
        ("X-Plex-Client-Identifier".into(), PLEX_CLIENT_IDENTIFIER.into()),
        ("X-Plex-Platform".into(), PLEX_PLATFORM.into()),
        ("X-Plex-Device".into(), PLEX_DEVICE.into()),
        ("X-Plex-Device-Name".into(), PLEX_DEVICE.into()),
        ("X-Plex-Version".into(), PLEX_VERSION.into()),
        ("Accept".into(), "application/json".into()),
    ];
    if let Some(t) = token {
        headers.push(("X-Plex-Token".into(), t.into()));
    }
    headers
}
