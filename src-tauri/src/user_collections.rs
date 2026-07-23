//! Local user-defined collections scoped by Plex server + library section.

use crate::error::{AppError, AppResult};
use crate::plex::models::UserCollection;
use crate::storage::app_data_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

const FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryCollections {
    collections: Vec<UserCollection>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerFile {
    version: u32,
    /// libraryKey → collections
    by_library: HashMap<String, LibraryCollections>,
}

fn safe_segment(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn file_path(server_id: &str) -> AppResult<PathBuf> {
    let dir = app_data_dir()?.join("user-collections");
    fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{}.json", safe_segment(server_id))))
}

fn load_server(server_id: &str) -> AppResult<ServerFile> {
    let path = file_path(server_id)?;
    if !path.exists() {
        return Ok(ServerFile {
            version: FILE_VERSION,
            by_library: HashMap::new(),
        });
    }
    let raw = fs::read_to_string(&path)?;
    let mut f: ServerFile = serde_json::from_str(&raw)
        .map_err(|e| AppError::Message(format!("invalid user collections file: {e}")))?;
    f.version = FILE_VERSION;
    Ok(f)
}

fn save_server(server_id: &str, f: &ServerFile) -> AppResult<()> {
    let path = file_path(server_id)?;
    let raw = serde_json::to_string_pretty(f)
        .map_err(|e| AppError::Message(e.to_string()))?;
    fs::write(path, raw)?;
    Ok(())
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn list(server_id: &str, library_key: &str) -> AppResult<Vec<UserCollection>> {
    let f = load_server(server_id)?;
    let mut list = f
        .by_library
        .get(library_key)
        .map(|l| l.collections.clone())
        .unwrap_or_default();
    list.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    Ok(list)
}

pub fn create(server_id: &str, library_key: &str, name: &str) -> AppResult<UserCollection> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Message("collection name required".into()));
    }
    let mut f = load_server(server_id)?;
    let entry = f
        .by_library
        .entry(library_key.to_string())
        .or_default();
    let ts = now();
    let col = UserCollection {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        rating_keys: vec![],
        created_at: ts.clone(),
        updated_at: ts,
    };
    entry.collections.push(col.clone());
    save_server(server_id, &f)?;
    Ok(col)
}

pub fn rename(
    server_id: &str,
    library_key: &str,
    id: &str,
    name: &str,
) -> AppResult<UserCollection> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Message("collection name required".into()));
    }
    let mut f = load_server(server_id)?;
    let entry = f
        .by_library
        .get_mut(library_key)
        .ok_or_else(|| AppError::Message("library has no collections".into()))?;
    let col = entry
        .collections
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::Message("collection not found".into()))?;
    col.name = name.to_string();
    col.updated_at = now();
    let out = col.clone();
    save_server(server_id, &f)?;
    Ok(out)
}

pub fn delete(server_id: &str, library_key: &str, id: &str) -> AppResult<()> {
    let mut f = load_server(server_id)?;
    if let Some(entry) = f.by_library.get_mut(library_key) {
        entry.collections.retain(|c| c.id != id);
        save_server(server_id, &f)?;
    }
    Ok(())
}

pub fn add_books(
    server_id: &str,
    library_key: &str,
    id: &str,
    rating_keys: Vec<String>,
) -> AppResult<UserCollection> {
    let mut f = load_server(server_id)?;
    let entry = f
        .by_library
        .get_mut(library_key)
        .ok_or_else(|| AppError::Message("library has no collections".into()))?;
    let col = entry
        .collections
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::Message("collection not found".into()))?;
    for k in rating_keys {
        if !k.is_empty() && !col.rating_keys.iter().any(|x| x == &k) {
            col.rating_keys.push(k);
        }
    }
    col.updated_at = now();
    let out = col.clone();
    save_server(server_id, &f)?;
    Ok(out)
}

pub fn remove_books(
    server_id: &str,
    library_key: &str,
    id: &str,
    rating_keys: Vec<String>,
) -> AppResult<UserCollection> {
    let mut f = load_server(server_id)?;
    let entry = f
        .by_library
        .get_mut(library_key)
        .ok_or_else(|| AppError::Message("library has no collections".into()))?;
    let col = entry
        .collections
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or_else(|| AppError::Message("collection not found".into()))?;
    col.rating_keys
        .retain(|k| !rating_keys.iter().any(|r| r == k));
    col.updated_at = now();
    let out = col.clone();
    save_server(server_id, &f)?;
    Ok(out)
}

pub fn get(server_id: &str, library_key: &str, id: &str) -> AppResult<Option<UserCollection>> {
    Ok(list(server_id, library_key)?
        .into_iter()
        .find(|c| c.id == id))
}
