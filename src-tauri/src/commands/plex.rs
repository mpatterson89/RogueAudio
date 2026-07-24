use crate::error::AppResult;
use crate::plex::{
    self, AudiobookSummary, AuthStatus, BookDetail, PlaybackInfo, PinAuthPoll, PinAuthStart,
    PlexCollection, PlexLibrary, PlexServer, StreamInfo,
};
use crate::PlexAuthState;
use tauri::State;

#[tauri::command]
pub async fn plex_start_pin_auth(state: State<'_, PlexAuthState>) -> AppResult<PinAuthStart> {
    plex::start_pin_auth(&state).await
}

#[tauri::command]
pub async fn plex_poll_pin_auth(state: State<'_, PlexAuthState>) -> AppResult<PinAuthPoll> {
    plex::poll_pin_auth(&state).await
}

#[tauri::command]
pub fn plex_logout() -> AppResult<()> {
    plex::logout()
}

#[tauri::command]
pub async fn plex_auth_status() -> AppResult<AuthStatus> {
    plex::auth_status().await
}

/// Debug-only helper so the UI can be exercised without a live Plex account.
#[tauri::command]
pub fn plex_dev_complete_auth(
    state: State<'_, PlexAuthState>,
    username: Option<String>,
) -> AppResult<AuthStatus> {
    #[cfg(debug_assertions)]
    {
        return plex::dev_complete_pin_auth(&state, username);
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = (state, username);
        Err(crate::error::AppError::NotImplemented(
            "dev auth is only available in debug builds",
        ))
    }
}

#[tauri::command]
pub async fn plex_list_servers() -> AppResult<Vec<PlexServer>> {
    if !plex::is_authenticated() {
        return Err(crate::error::AppError::NotAuthenticated);
    }
    plex::list_servers().await
}

/// List music-type libraries (audiobook sources) on a server.
#[tauri::command]
pub async fn plex_list_libraries(server_id: String) -> AppResult<Vec<PlexLibrary>> {
    if !plex::is_authenticated() {
        return Err(crate::error::AppError::NotAuthenticated);
    }
    plex::list_libraries(&server_id).await
}

/// List or search albums (books) in a music library section.
#[tauri::command]
pub async fn plex_list_books(
    server_id: String,
    library_key: String,
    query: Option<String>,
) -> AppResult<Vec<AudiobookSummary>> {
    if !plex::is_authenticated() {
        return Err(crate::error::AppError::NotAuthenticated);
    }
    plex::list_books(&server_id, &library_key, query.as_deref()).await
}

/// Plex collections on a music/audiobook library section.
#[tauri::command]
pub async fn plex_list_collections(
    server_id: String,
    library_key: String,
) -> AppResult<Vec<PlexCollection>> {
    if !plex::is_authenticated() {
        return Err(crate::error::AppError::NotAuthenticated);
    }
    plex::list_collections(&server_id, &library_key).await
}

/// Books inside a Plex collection.
#[tauri::command]
pub async fn plex_collection_books(
    server_id: String,
    collection_rating_key: String,
) -> AppResult<Vec<AudiobookSummary>> {
    if !plex::is_authenticated() {
        return Err(crate::error::AppError::NotAuthenticated);
    }
    plex::collection_books(&server_id, &collection_rating_key).await
}

/// Full book detail for the book view (summary, art, chapters).
#[tauri::command]
pub async fn plex_get_book_detail(
    server_id: String,
    rating_key: String,
) -> AppResult<BookDetail> {
    if !plex::is_authenticated() {
        return Err(crate::error::AppError::NotAuthenticated);
    }
    plex::get_book_detail(&server_id, &rating_key).await
}

/// Resolve a book/album (or track) into an ordered list of stream URLs.
#[tauri::command]
pub async fn plex_get_playback(
    server_id: String,
    rating_key: String,
) -> AppResult<PlaybackInfo> {
    if !plex::is_authenticated() {
        return Err(crate::error::AppError::NotAuthenticated);
    }
    plex::get_playback(&server_id, &rating_key).await
}

/// Convenience: first track stream only (legacy / simple clients).
#[tauri::command]
pub async fn plex_get_stream(
    server_id: String,
    rating_key: String,
) -> AppResult<StreamInfo> {
    let playback = plex_get_playback(server_id, rating_key).await?;
    let first = playback
        .tracks
        .into_iter()
        .next()
        .ok_or_else(|| crate::error::AppError::Message("no tracks".into()))?;
    Ok(StreamInfo {
        url: first.url,
        headers: vec![],
        duration_ms: first.duration_ms.or(playback.total_duration_ms),
        container: first.container,
    })
}
