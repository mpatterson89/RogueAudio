use crate::error::AppResult;
use crate::plex::models::UserCollection;
use crate::user_collections;

#[tauri::command]
pub fn user_collections_list(
    server_id: String,
    library_key: String,
) -> AppResult<Vec<UserCollection>> {
    user_collections::list(&server_id, &library_key)
}

#[tauri::command]
pub fn user_collections_create(
    server_id: String,
    library_key: String,
    name: String,
) -> AppResult<UserCollection> {
    user_collections::create(&server_id, &library_key, &name)
}

#[tauri::command]
pub fn user_collections_rename(
    server_id: String,
    library_key: String,
    id: String,
    name: String,
) -> AppResult<UserCollection> {
    user_collections::rename(&server_id, &library_key, &id, &name)
}

#[tauri::command]
pub fn user_collections_delete(
    server_id: String,
    library_key: String,
    id: String,
) -> AppResult<()> {
    user_collections::delete(&server_id, &library_key, &id)
}

#[tauri::command]
pub fn user_collections_add_books(
    server_id: String,
    library_key: String,
    id: String,
    rating_keys: Vec<String>,
) -> AppResult<UserCollection> {
    user_collections::add_books(&server_id, &library_key, &id, rating_keys)
}

#[tauri::command]
pub fn user_collections_remove_books(
    server_id: String,
    library_key: String,
    id: String,
    rating_keys: Vec<String>,
) -> AppResult<UserCollection> {
    user_collections::remove_books(&server_id, &library_key, &id, rating_keys)
}

#[tauri::command]
pub fn user_collections_get(
    server_id: String,
    library_key: String,
    id: String,
) -> AppResult<Option<UserCollection>> {
    user_collections::get(&server_id, &library_key, &id)
}
