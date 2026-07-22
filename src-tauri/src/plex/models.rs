use serde::{Deserialize, Serialize};

/// Result of starting the Plex PIN authentication flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinAuthStart {
    pub id: u64,
    pub code: String,
    /// URL the user should open to authorize the PIN.
    pub auth_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub authenticated: bool,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinAuthPoll {
    pub authorized: bool,
    pub status: AuthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexServer {
    pub id: String,
    pub name: String,
    pub product: Option<String>,
    pub provides: Option<String>,
    pub public_address: Option<String>,
    #[serde(default)]
    pub owned: bool,
    pub connections: Vec<PlexConnection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexConnection {
    pub uri: String,
    pub local: bool,
    pub relay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexLibrary {
    pub key: String,
    pub title: String,
    /// Plex section type. Music / audiobook libraries use `"artist"`.
    pub library_type: String,
    pub agent: Option<String>,
}

impl PlexLibrary {
    /// Plex stores audiobook libraries as Music sections (`type=artist`).
    pub fn is_music_section(&self) -> bool {
        matches!(
            self.library_type.to_ascii_lowercase().as_str(),
            "artist" | "music"
        )
    }

    /// Prefer sections whose title looks like an audiobook library.
    pub fn looks_like_audiobooks(&self) -> bool {
        let t = self.title.to_ascii_lowercase();
        t.contains("audio") || t.contains("book") || t.contains("spoken")
    }
}

/// An audiobook-like item (often a music album / artist folder in Plex).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookSummary {
    pub rating_key: String,
    pub title: String,
    pub author: Option<String>,
    pub thumb: Option<String>,
    pub year: Option<i32>,
    pub duration_ms: Option<u64>,
    pub library_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamInfo {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub duration_ms: Option<u64>,
    pub container: Option<String>,
}

/// One playable audio part (usually a track under an album/book).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackTrack {
    pub rating_key: String,
    pub title: String,
    pub index: u32,
    pub duration_ms: Option<u64>,
    /// Fully-qualified stream URL including X-Plex-Token query param.
    pub url: String,
    pub container: Option<String>,
}

/// Resolved playlist for a book (album) or single track.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackInfo {
    pub book_rating_key: String,
    pub tracks: Vec<PlaybackTrack>,
    pub total_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressSnapshot {
    pub rating_key: String,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub updated_at: String,
    pub source: ProgressSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProgressSource {
    Local,
    Plex,
    Merged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressReport {
    pub rating_key: String,
    /// playing | paused | stopped | buffering
    pub state: String,
    pub time_ms: u64,
    pub duration_ms: Option<u64>,
    pub speed: f64,
}
