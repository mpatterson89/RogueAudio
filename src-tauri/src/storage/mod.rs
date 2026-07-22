pub mod config;

use crate::error::{AppError, AppResult};
use std::fs;
use std::path::PathBuf;

/// Root application data directory: `~/.local/share/rogue-audio` (or platform equivalent).
pub fn app_data_dir() -> AppResult<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| {
        AppError::Message("could not resolve platform data directory".into())
    })?;
    let path = base.join("rogue-audio");
    fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn config_path() -> AppResult<PathBuf> {
    Ok(app_data_dir()?.join("config.json"))
}
