use crate::error::AppResult;
use crate::plex::{
    self, AudiobookSummary, AuthStatus, PinAuthPoll, PinAuthStart, PlexLibrary, PlexServer,
    StreamInfo,
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
pub fn plex_auth_status() -> AppResult<AuthStatus> {
    plex::auth_status()
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
pub fn plex_list_servers() -> AppResult<Vec<PlexServer>> {
    let status = plex::auth_status()?;
    if !status.authenticated {
        return Err(crate::error::AppError::NotAuthenticated);
    }

    // Stub sample data for UI wiring until plex.tv resources API is connected.
    Ok(vec![PlexServer {
        id: "local-dev-server".into(),
        name: "Home Media (stub)".into(),
        product: Some("Plex Media Server".into()),
        provides: Some("server".into()),
        public_address: None,
        connections: vec![plex::PlexConnection {
            uri: "http://127.0.0.1:32400".into(),
            local: true,
            relay: false,
        }],
    }])
}

/// List libraries usable as audiobook sources.
///
/// Plex models audiobooks as **Music** sections (`type=artist`). We only return
/// music-type libraries. When more than one exists, the frontend shows a filter.
#[tauri::command]
pub fn plex_list_libraries(_server_id: String) -> AppResult<Vec<PlexLibrary>> {
    let status = plex::auth_status()?;
    if !status.authenticated {
        return Err(crate::error::AppError::NotAuthenticated);
    }

    // Stub: real PMS `/library/sections` will replace this list.
    let all = vec![
        PlexLibrary {
            key: "1".into(),
            title: "Audiobooks".into(),
            library_type: "artist".into(),
            agent: Some("com.plexapp.agents.none".into()),
        },
        PlexLibrary {
            key: "2".into(),
            title: "Music".into(),
            library_type: "artist".into(),
            agent: None,
        },
        // Non-music section — filtered out (movies/tv are not book sources).
        PlexLibrary {
            key: "3".into(),
            title: "Movies".into(),
            library_type: "movie".into(),
            agent: None,
        },
    ];

    Ok(filter_music_libraries(all))
}

/// Keep only music-type sections; sort audiobook-titled libraries first.
pub fn filter_music_libraries(libraries: Vec<PlexLibrary>) -> Vec<PlexLibrary> {
    let mut music: Vec<PlexLibrary> = libraries
        .into_iter()
        .filter(|l| l.is_music_section())
        .collect();
    music.sort_by_key(|l| (!l.looks_like_audiobooks(), l.title.to_ascii_lowercase()));
    music
}

#[tauri::command]
pub fn plex_list_books(
    _server_id: String,
    library_key: String,
    query: Option<String>,
) -> AppResult<Vec<AudiobookSummary>> {
    let status = plex::auth_status()?;
    if !status.authenticated {
        return Err(crate::error::AppError::NotAuthenticated);
    }

    let books = sample_books(&library_key);
    let filtered = match query {
        Some(q) if !q.trim().is_empty() => {
            let q = q.to_lowercase();
            books
                .into_iter()
                .filter(|b| {
                    b.title.to_lowercase().contains(&q)
                        || b.author
                            .as_ref()
                            .map(|a| a.to_lowercase().contains(&q))
                            .unwrap_or(false)
                })
                .collect()
        }
        _ => books,
    };
    Ok(filtered)
}

#[tauri::command]
pub fn plex_get_stream(
    _server_id: String,
    rating_key: String,
) -> AppResult<StreamInfo> {
    let status = plex::auth_status()?;
    if !status.authenticated {
        return Err(crate::error::AppError::NotAuthenticated);
    }

    // Placeholder: real implementation resolves /library/metadata/{id}/... part path.
    Ok(StreamInfo {
        url: format!("plex://stub/stream/{rating_key}"),
        headers: vec![("X-Plex-Token".into(), "dev-token-not-for-production".into())],
        duration_ms: Some(12 * 60 * 60 * 1000),
        container: Some("m4b".into()),
    })
}

fn sample_books(library_key: &str) -> Vec<AudiobookSummary> {
    vec![
        AudiobookSummary {
            rating_key: "1001".into(),
            title: "The Name of the Wind".into(),
            author: Some("Patrick Rothfuss".into()),
            thumb: None,
            year: Some(2007),
            duration_ms: Some(27 * 60 * 60 * 1000 + 55 * 60 * 1000),
            library_key: Some(library_key.into()),
        },
        AudiobookSummary {
            rating_key: "1002".into(),
            title: "Project Hail Mary".into(),
            author: Some("Andy Weir".into()),
            thumb: None,
            year: Some(2021),
            duration_ms: Some(16 * 60 * 60 * 1000 + 10 * 60 * 1000),
            library_key: Some(library_key.into()),
        },
        AudiobookSummary {
            rating_key: "1003".into(),
            title: "Dune".into(),
            author: Some("Frank Herbert".into()),
            thumb: None,
            year: Some(1965),
            duration_ms: Some(21 * 60 * 60 * 1000),
            library_key: Some(library_key.into()),
        },
        AudiobookSummary {
            rating_key: "1004".into(),
            title: "The Hobbit".into(),
            author: Some("J.R.R. Tolkien".into()),
            thumb: None,
            year: Some(1937),
            duration_ms: Some(11 * 60 * 60 * 1000 + 8 * 60 * 1000),
            library_key: Some(library_key.into()),
        },
        AudiobookSummary {
            rating_key: "1005".into(),
            title: "Neuromancer".into(),
            author: Some("William Gibson".into()),
            thumb: None,
            year: Some(1984),
            duration_ms: Some(10 * 60 * 60 * 1000 + 30 * 60 * 1000),
            library_key: Some(library_key.into()),
        },
        AudiobookSummary {
            rating_key: "1006".into(),
            title: "Children of Time".into(),
            author: Some("Adrian Tchaikovsky".into()),
            thumb: None,
            year: Some(2015),
            duration_ms: Some(16 * 60 * 60 * 1000 + 30 * 60 * 1000),
            library_key: Some(library_key.into()),
        },
    ]
}
