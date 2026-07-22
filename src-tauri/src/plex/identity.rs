//! Plex client identity used in auth and API headers.
//! Keep product name stable; client identifier is a per-install UUID (see storage).

/// Display / product name shown in Plex "authorized devices".
pub const PLEX_PRODUCT: &str = "RogueAudio";

/// Fallback reverse-DNS style id (real requests use the per-install UUID).
pub const PLEX_CLIENT_IDENTIFIER: &str = "app.rogueaudio";

/// Platform string for X-Plex-Platform.
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

/// Version sent as X-Plex-Version.
pub const PLEX_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Standard X-Plex-* headers for plex.tv and PMS requests.
/// Note: `X-Plex-Client-Identifier` is overwritten with the per-install UUID by the HTTP client.
pub fn plex_headers(token: Option<&str>) -> Vec<(String, String)> {
    let mut headers = vec![
        ("X-Plex-Product".into(), PLEX_PRODUCT.into()),
        (
            "X-Plex-Client-Identifier".into(),
            PLEX_CLIENT_IDENTIFIER.into(),
        ),
        ("X-Plex-Platform".into(), PLEX_PLATFORM.into()),
        ("X-Plex-Device".into(), PLEX_DEVICE.into()),
        ("X-Plex-Device-Name".into(), PLEX_DEVICE.into()),
        ("X-Plex-Version".into(), PLEX_VERSION.into()),
        ("X-Plex-Model".into(), "hosted".into()),
        ("Accept".into(), "application/json".into()),
    ];
    if let Some(t) = token {
        headers.push(("X-Plex-Token".into(), t.into()));
    }
    headers
}
