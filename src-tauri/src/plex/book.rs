//! Book detail: summary, art, and chapter list for the book view.

use super::client::pms_get_json;
use super::models::{BookChapter, BookDetail};
use super::server::{absolute_media_url, connect, ServerSession};
use crate::error::{AppError, AppResult};
use serde_json::Value;

pub async fn get_book_detail(server_id: &str, rating_key: &str) -> AppResult<BookDetail> {
    let session = connect(server_id).await?;
    let meta = fetch_metadata(&session, rating_key).await?;

    let meta_type = meta
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // Prefer album-level node for summary/art; if a track was passed, resolve parent album.
    let album = if meta_type == "track" {
        if let Some(parent_key) = meta
            .get("parentRatingKey")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        {
            fetch_metadata(&session, &parent_key)
                .await
                .unwrap_or(meta.clone())
        } else {
            meta.clone()
        }
    } else {
        meta.clone()
    };

    let album_key = album
        .get("ratingKey")
        .and_then(|v| v.as_str())
        .unwrap_or(rating_key)
        .to_string();

    let title = album
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string();

    let author = album
        .get("parentTitle")
        .and_then(|v| v.as_str())
        .or_else(|| album.get("originalTitle").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    let summary = album
        .get("summary")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let year = album.get("year").and_then(|v| v.as_i64()).map(|y| y as i32);
    let studio = album
        .get("studio")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    // Series: first Collection tag. Index: title "Book N", collection order, then album index.
    let series = extract_series_name(&album);
    let library_key_for_series = album
        .get("librarySectionID")
        .map(|v| match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => String::new(),
        })
        .filter(|s| !s.is_empty());
    let series_index = extract_series_index(
        &session,
        &album,
        &album_key,
        &title,
        series.as_deref(),
        library_key_for_series.as_deref(),
    )
    .await;

    let duration_ms = album.get("duration").and_then(|v| v.as_u64());

    let thumb = album
        .get("thumb")
        .and_then(|v| v.as_str())
        .map(|p| absolute_media_url(&session.base_uri, p, &session.token));

    let art = album
        .get("art")
        .and_then(|v| v.as_str())
        .or_else(|| album.get("thumb").and_then(|v| v.as_str()))
        .map(|p| absolute_media_url(&session.base_uri, p, &session.token));

    let library_key = album
        .get("librarySectionID")
        .map(|v| match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => String::new(),
        })
        .filter(|s| !s.is_empty());

    // Children tracks for multi-file books + chapter extraction
    let tracks = fetch_children(&session, &album_key)
        .await
        .unwrap_or_default();

    let track_nodes: Vec<Value> = if tracks.is_empty() {
        // Single-item album (media on album) or direct track
        if has_media(&album) {
            vec![album.clone()]
        } else if meta_type == "track" {
            vec![meta]
        } else {
            vec![]
        }
    } else {
        tracks
    };

    let chapters = build_chapters(&session, &track_nodes).await;
    let track_count = track_nodes.len() as u32;

    // Prefer summed chapter span / track durations for total length
    let duration_ms = duration_ms.or_else(|| {
        let sum: u64 = track_nodes
            .iter()
            .filter_map(|t| t.get("duration").and_then(|v| v.as_u64()))
            .sum();
        if sum > 0 { Some(sum) } else { None }
    });

    Ok(BookDetail {
        rating_key: album_key,
        title,
        author,
        summary,
        year,
        thumb,
        art,
        duration_ms,
        library_key,
        studio,
        series,
        series_index,
        chapters,
        track_count,
    })
}

/// First non-empty Collection tag (common place series titles live in Plex).
fn extract_series_name(meta: &Value) -> Option<String> {
    let collections = meta.get("Collection")?.as_array()?;
    for c in collections {
        if let Some(tag) = c.get("tag").and_then(|v| v.as_str()) {
            let t = tag.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// Plex album `index` is almost always 1 for standalone audiobook albums, so it is a
/// weak signal. Prefer explicit "Book N" in the title, then position within the
/// Plex collection (sorted by year), then album index only when it is > 1.
async fn extract_series_index(
    session: &ServerSession,
    album: &Value,
    album_key: &str,
    title: &str,
    series: Option<&str>,
    library_key: Option<&str>,
) -> Option<u32> {
    if let Some(n) = parse_book_number_from_title(title) {
        return Some(n);
    }

    if series.is_some() {
        if let Some(n) =
            series_index_from_collection(session, album, album_key, library_key).await
        {
            return Some(n);
        }
    }

    album
        .get("index")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .filter(|&n| n > 1)
}

/// "Dungeons and Noobs: Noobtown, Book 4", "Noobtown #5", "Vol. 3", etc.
fn parse_book_number_from_title(title: &str) -> Option<u32> {
    let t = title.trim();
    // Book 4 / book 04 / Book4
    let book_re = regex_lite_book_n(t);
    if book_re.is_some() {
        return book_re;
    }
    // #4 or # 4 (avoid matching years)
    if let Some(cap) = find_hash_number(t) {
        return Some(cap);
    }
    // Vol. 3 / Volume 3
    if let Some(cap) = find_vol_number(t) {
        return Some(cap);
    }
    None
}

fn regex_lite_book_n(title: &str) -> Option<u32> {
    let lower = title.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let needle = b"book";
    let mut i = 0;
    while i + needle.len() < bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let after = &lower[i + needle.len()..];
            let after = after.trim_start_matches(|c: char| c == ' ' || c == '.' || c == ':' || c == '-');
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<u32>() {
                if (1..=999).contains(&n) {
                    return Some(n);
                }
            }
        }
        i += 1;
    }
    None
}

fn find_hash_number(title: &str) -> Option<u32> {
    let bytes = title.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'#' {
            continue;
        }
        let rest = title[i + 1..].trim_start();
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = digits.parse::<u32>() {
            if (1..=999).contains(&n) {
                return Some(n);
            }
        }
    }
    None
}

fn find_vol_number(title: &str) -> Option<u32> {
    let lower = title.to_ascii_lowercase();
    for needle in ["volume", "vol."] {
        if let Some(pos) = lower.find(needle) {
            let after = lower[pos + needle.len()..].trim_start_matches(|c: char| {
                c == ' ' || c == '.' || c == ':' || c == '-'
            });
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<u32>() {
                if (1..=999).contains(&n) {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// Position (1-based) of this album in its Plex collection, sorted by year then title.
async fn series_index_from_collection(
    session: &ServerSession,
    album: &Value,
    album_key: &str,
    library_key: Option<&str>,
) -> Option<u32> {
    let collection_id = album
        .get("Collection")?
        .as_array()?
        .first()?
        .get("id")
        .and_then(|v| match v {
            Value::Number(n) => n.as_u64().map(|u| u.to_string()),
            Value::String(s) => Some(s.clone()),
            _ => None,
        })?;

    let section = library_key
        .map(|s| s.to_string())
        .or_else(|| {
            album
                .get("librarySectionID")
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    _ => String::new(),
                })
                .filter(|s| !s.is_empty())
        })?;

    // type=9 = album. sort=year matches publication order for most series.
    let path = format!(
        "/library/sections/{section}/all?type=9&collection={collection_id}&sort=year,titleSort"
    );
    let raw: Value = pms_get_json(&session.base_uri, &path, &session.token)
        .await
        .ok()?;
    let items = raw
        .pointer("/MediaContainer/Metadata")?
        .as_array()?;

    for (i, item) in items.iter().enumerate() {
        let rk = item.get("ratingKey").and_then(|v| v.as_str()).unwrap_or("");
        if rk == album_key {
            return Some((i as u32) + 1);
        }
    }
    None
}

async fn fetch_metadata(session: &ServerSession, rating_key: &str) -> AppResult<Value> {
    let path = format!(
        "/library/metadata/{rating_key}?includeChapters=1&includeCollections=1&includeFields=summary,art,thumb,title,year,studio,duration,parentTitle,originalTitle,index,Collection"
    );
    let raw: Value = pms_get_json(&session.base_uri, &path, &session.token).await?;
    raw.pointer("/MediaContainer/Metadata/0")
        .cloned()
        .ok_or_else(|| AppError::Message(format!("metadata not found for {rating_key}")))
}

async fn fetch_children(session: &ServerSession, rating_key: &str) -> AppResult<Vec<Value>> {
    let path = format!("/library/metadata/{rating_key}/children");
    let raw: Value = pms_get_json(&session.base_uri, &path, &session.token).await?;
    Ok(raw
        .pointer("/MediaContainer/Metadata")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default())
}

fn has_media(meta: &Value) -> bool {
    meta.get("Media")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
}

async fn build_chapters(session: &ServerSession, tracks: &[Value]) -> Vec<BookChapter> {
    let mut chapters: Vec<BookChapter> = Vec::new();
    let mut book_offset_ms: u64 = 0;
    let mut chapter_index: u32 = 0;

    for (ti, track) in tracks.iter().enumerate() {
        let track_key = track
            .get("ratingKey")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let track_title = track
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Track")
            .to_string();
        let track_duration = track.get("duration").and_then(|v| v.as_u64()).unwrap_or(0);

        // Embedded chapters live on the track metadata when chapterSource=media
        let embedded = if !track_key.is_empty() {
            fetch_track_chapters(session, track_key)
                .await
                .unwrap_or_default()
        } else {
            extract_chapters_from_node(track)
        };

        if !embedded.is_empty() {
            for ch in embedded {
                let start = book_offset_ms.saturating_add(ch.start_ms);
                let end = ch.end_ms.map(|e| book_offset_ms.saturating_add(e));
                chapters.push(BookChapter {
                    index: chapter_index,
                    title: ch.title,
                    start_ms: start,
                    end_ms: end,
                    source: "embedded".into(),
                });
                chapter_index += 1;
            }
        } else if tracks.len() > 1 {
            // Multi-file book without markers → treat each file as a chapter
            chapters.push(BookChapter {
                index: chapter_index,
                title: if track_title.is_empty() {
                    format!("Part {}", ti + 1)
                } else {
                    track_title
                },
                start_ms: book_offset_ms,
                end_ms: if track_duration > 0 {
                    Some(book_offset_ms + track_duration)
                } else {
                    None
                },
                source: "track".into(),
            });
            chapter_index += 1;
        }

        book_offset_ms = book_offset_ms.saturating_add(track_duration);
    }

    // Single continuous file, no chapters — empty list is fine (UI shows empty state)
    chapters
}

struct RawChapter {
    title: String,
    start_ms: u64,
    end_ms: Option<u64>,
}

async fn fetch_track_chapters(session: &ServerSession, rating_key: &str) -> AppResult<Vec<RawChapter>> {
    let path = format!("/library/metadata/{rating_key}?includeChapters=1");
    let raw: Value = pms_get_json(&session.base_uri, &path, &session.token).await?;
    let node = raw
        .pointer("/MediaContainer/Metadata/0")
        .cloned()
        .unwrap_or(Value::Null);
    Ok(extract_chapters_from_node(&node))
}

fn extract_chapters_from_node(node: &Value) -> Vec<RawChapter> {
    let mut out = Vec::new();

    // Top-level Chapter[] on metadata
    if let Some(arr) = node.get("Chapter").and_then(|v| v.as_array()) {
        for ch in arr {
            if let Some(rc) = parse_chapter(ch) {
                out.push(rc);
            }
        }
    }

    // Fallback: Part.Chapter[]
    if out.is_empty() {
        if let Some(media) = node.get("Media").and_then(|v| v.as_array()) {
            for m in media {
                if let Some(parts) = m.get("Part").and_then(|v| v.as_array()) {
                    for p in parts {
                        if let Some(arr) = p.get("Chapter").and_then(|v| v.as_array()) {
                            for ch in arr {
                                if let Some(rc) = parse_chapter(ch) {
                                    out.push(rc);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    out
}

fn parse_chapter(ch: &Value) -> Option<RawChapter> {
    let start = json_u64(ch.get("startTimeOffset"))?;
    let end = json_u64(ch.get("endTimeOffset"));
    let index = json_u64(ch.get("index")).unwrap_or(0);
    // Plex audiobook chapters use `tag` for the name (e.g. "Opening Credits").
    let title = ch
        .get("tag")
        .and_then(|v| v.as_str())
        .or_else(|| ch.get("title").and_then(|v| v.as_str()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("Chapter {index}"));

    Some(RawChapter {
        title,
        start_ms: start,
        end_ms: end,
    })
}

fn json_u64(v: Option<&Value>) -> Option<u64> {
    let v = v?;
    v.as_u64()
        .or_else(|| v.as_i64().map(|i| i.max(0) as u64))
        .or_else(|| v.as_f64().map(|f| f.max(0.0) as u64))
        .or_else(|| v.as_str()?.parse().ok())
}
