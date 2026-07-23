//! On-disk album cover cache for library grid + book view.
//!
//! Layout: `~/.local/share/rogue-audio/covers/{serverId}/{ratingKey}.jpg`

use crate::error::{AppError, AppResult};
use crate::storage::app_data_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn safe_segment(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if t.is_empty() {
        "_".into()
    } else {
        t
    }
}

pub fn covers_root() -> AppResult<PathBuf> {
    let dir = app_data_dir()?.join("covers");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn server_dir(server_id: &str) -> AppResult<PathBuf> {
    let dir = covers_root()?.join(safe_segment(server_id));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Preferred cover path (jpg). May not exist yet.
pub fn cover_path(server_id: &str, rating_key: &str) -> AppResult<PathBuf> {
    Ok(server_dir(server_id)?.join(format!("{}.jpg", safe_segment(rating_key))))
}

/// Any existing cover file for this title (jpg/png/webp).
pub fn find_local(server_id: &str, rating_key: &str) -> AppResult<Option<PathBuf>> {
    let dir = server_dir(server_id)?;
    let base = safe_segment(rating_key);
    for ext in ["jpg", "jpeg", "png", "webp", "gif"] {
        let p = dir.join(format!("{base}.{ext}"));
        if p.is_file() {
            return Ok(Some(p));
        }
    }
    // Shared with offline download folder
    let dl = app_data_dir()?
        .join("downloads")
        .join(safe_segment(rating_key))
        .join("cover.jpg");
    if dl.is_file() {
        return Ok(Some(dl));
    }
    Ok(None)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverEnsureRequest {
    pub server_id: String,
    pub rating_key: String,
    pub remote_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverEnsureResult {
    pub rating_key: String,
    /// Absolute filesystem path when successful.
    pub path: Option<String>,
    pub error: Option<String>,
}

fn http_client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(format!(
            "RogueAudio/{} (covers)",
            env!("CARGO_PKG_VERSION")
        ))
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Message(format!("cover client: {e}")))
}

fn ext_from_content_type(ct: Option<&str>) -> &'static str {
    match ct.unwrap_or("").to_ascii_lowercase().as_str() {
        t if t.contains("png") => "png",
        t if t.contains("webp") => "webp",
        t if t.contains("gif") => "gif",
        t if t.contains("jpeg") || t.contains("jpg") => "jpg",
        _ => "jpg",
    }
}

/// Copy an existing image into the shared cover cache (e.g. from offline download).
pub fn import_file(server_id: &str, rating_key: &str, src: &Path) -> AppResult<String> {
    if !src.is_file() {
        return Err(AppError::Message(format!(
            "cover source missing: {}",
            src.display()
        )));
    }
    let dest = cover_path(server_id, rating_key)?;
    if dest.exists() {
        // Already cached
        return Ok(dest.to_string_lossy().to_string());
    }
    fs::copy(src, &dest)?;
    Ok(dest.to_string_lossy().to_string())
}

/// Return local path if present; otherwise download `remote_url` and store it.
pub async fn ensure(server_id: &str, rating_key: &str, remote_url: &str) -> AppResult<String> {
    if let Some(existing) = find_local(server_id, rating_key)? {
        return Ok(existing.to_string_lossy().to_string());
    }

    let url = remote_url.trim();
    if url.is_empty() {
        return Err(AppError::Message("empty cover url".into()));
    }

    // Absolute local path already (from prior convert) — nothing to download
    if url.starts_with('/') && Path::new(url).is_file() {
        return import_file(server_id, rating_key, Path::new(url));
    }

    let client = http_client()?;
    let res = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Message(format!("cover download failed: {e}")))?;

    if !res.status().is_success() {
        return Err(AppError::Message(format!(
            "cover HTTP {}",
            res.status()
        )));
    }

    let ct = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let ext = ext_from_content_type(ct.as_deref());

    let bytes = res
        .bytes()
        .await
        .map_err(|e| AppError::Message(format!("cover body: {e}")))?;
    if bytes.is_empty() {
        return Err(AppError::Message("empty cover image".into()));
    }

    let dest = if ext == "jpg" {
        cover_path(server_id, rating_key)?
    } else {
        server_dir(server_id)?.join(format!("{}.{}", safe_segment(rating_key), ext))
    };

    let tmp = dest.with_extension(format!("{ext}.partial"));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.flush()?;
    }
    if dest.exists() {
        let _ = fs::remove_file(&dest);
    }
    fs::rename(&tmp, &dest)?;
    Ok(dest.to_string_lossy().to_string())
}

/// Ensure many covers; continues on individual failures.
pub async fn ensure_many(reqs: Vec<CoverEnsureRequest>) -> Vec<CoverEnsureResult> {
    let mut out = Vec::with_capacity(reqs.len());
    // Sequential is safer for PMS; FE can chunk concurrency if needed
    for r in reqs {
        match ensure(&r.server_id, &r.rating_key, &r.remote_url).await {
            Ok(path) => out.push(CoverEnsureResult {
                rating_key: r.rating_key,
                path: Some(path),
                error: None,
            }),
            Err(e) => out.push(CoverEnsureResult {
                rating_key: r.rating_key,
                path: None,
                error: Some(e.to_string()),
            }),
        }
    }
    out
}
