//! Local HTTP media server for offline playback.
//!
//! Tauri's `asset:` protocol + WebKit often fail on large AAC/M4B files (no Range,
//! wrong MIME, or missing codecs). Serving downloads from `http://127.0.0.1:<port>/…`
//! with proper `Content-Type` and HTTP Range works with HTML5 `<audio>`.

use crate::downloads::downloads_root;
use crate::error::{AppError, AppResult};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::OnceLock;
use std::thread;

static PORT: AtomicU16 = AtomicU16::new(0);
static STARTED: OnceLock<()> = OnceLock::new();

/// Bound port (0 if not started).
pub fn port() -> u16 {
    PORT.load(Ordering::SeqCst)
}

/// Base URL e.g. `http://127.0.0.1:43123` or empty if down.
pub fn base_url() -> Option<String> {
    let p = port();
    if p == 0 {
        None
    } else {
        Some(format!("http://127.0.0.1:{p}"))
    }
}

/// Public URL for a file under the downloads root.
pub fn url_for_download_file(absolute_path: &Path) -> AppResult<String> {
    let root = downloads_root()?;
    let abs = absolute_path
        .canonicalize()
        .unwrap_or_else(|_| absolute_path.to_path_buf());
    let root = root.canonicalize().unwrap_or(root);
    let rel = abs
        .strip_prefix(&root)
        .map_err(|_| AppError::Message("media path outside downloads root".into()))?;
    let mut segs = Vec::new();
    for c in rel.components() {
        use std::path::Component;
        match c {
            Component::Normal(s) => segs.push(urlencoding_encode(&s.to_string_lossy())),
            _ => {
                return Err(AppError::Message("invalid media path".into()));
            }
        }
    }
    let base = base_url().ok_or_else(|| AppError::Message("media server not started".into()))?;
    Ok(format!("{base}/d/{}", segs.join("/")))
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn urlencoding_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Start the media server once (idempotent). Call from app setup.
pub fn start() -> AppResult<u16> {
    if let Some(p) = base_url().map(|_| port()) {
        if p != 0 {
            return Ok(p);
        }
    }
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| AppError::Message(format!("media server bind: {e}")))?;
    let p = listener
        .local_addr()
        .map_err(|e| AppError::Message(e.to_string()))?
        .port();
    PORT.store(p, Ordering::SeqCst);
    STARTED.get_or_init(|| ());

    thread::Builder::new()
        .name("ra-media-server".into())
        .spawn(move || {
            for conn in listener.incoming() {
                match conn {
                    Ok(stream) => {
                        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
                        let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(120)));
                        if let Err(e) = handle_client(stream) {
                            eprintln!("media server: {e}");
                        }
                    }
                    Err(e) => eprintln!("media server accept: {e}"),
                }
            }
        })
        .map_err(|e| AppError::Message(format!("media server thread: {e}")))?;

    eprintln!("RogueAudio media server on http://127.0.0.1:{p}");
    Ok(p)
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mp3" | "mpeg" => "audio/mpeg",
        "m4a" | "m4b" | "mp4" | "aac" => "audio/mp4",
        "ogg" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        _ => "application/octet-stream",
    }
}

fn handle_client(mut stream: TcpStream) -> std::io::Result<()> {
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf)?;
    if n == 0 {
        return Ok(());
    }
    let req = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let path_q = parts.next().unwrap_or("/");

    if method != "GET" && method != "HEAD" {
        return write_response(&mut stream, 405, "text/plain", b"Method Not Allowed", None);
    }

    // Parse Range header
    let mut range: Option<(u64, Option<u64>)> = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("range:") {
            let rest = rest.trim();
            if let Some(spec) = rest.strip_prefix("bytes=") {
                let mut segs = spec.splitn(2, '-');
                let start: u64 = segs.next().unwrap_or("0").parse().unwrap_or(0);
                let end = segs
                    .next()
                    .filter(|s| !s.is_empty())
                    .and_then(|s| s.parse().ok());
                range = Some((start, end));
            }
        }
    }

    // Only /d/<relpath>
    let path_only = path_q.split('?').next().unwrap_or(path_q);
    if !path_only.starts_with("/d/") {
        return write_response(&mut stream, 404, "text/plain", b"Not Found", None);
    }
    let rel = &path_only[3..];
    let decoded: PathBuf = rel
        .split('/')
        .filter(|s| !s.is_empty())
        .map(urlencoding_decode)
        .collect();

    // Reject path traversal
    if decoded
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return write_response(&mut stream, 400, "text/plain", b"Bad Request", None);
    }

    let root = match downloads_root() {
        Ok(r) => r,
        Err(_) => {
            return write_response(&mut stream, 500, "text/plain", b"Server Error", None);
        }
    };
    let file_path = root.join(&decoded);
    let file_path = match file_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return write_response(&mut stream, 404, "text/plain", b"Not Found", None);
        }
    };
    let root_c = root.canonicalize().unwrap_or(root);
    if !file_path.starts_with(&root_c) || !file_path.is_file() {
        return write_response(&mut stream, 404, "text/plain", b"Not Found", None);
    }

    let mut file = File::open(&file_path)?;
    let total = file.metadata()?.len();
    let mime = mime_for(&file_path);

    if method == "HEAD" {
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {total}\r\nAccept-Ranges: bytes\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n"
        );
        stream.write_all(headers.as_bytes())?;
        return Ok(());
    }

    let (status, start, end) = if let Some((rs, re)) = range {
        let end = re.unwrap_or(total.saturating_sub(1)).min(total.saturating_sub(1));
        if rs >= total {
            let body = b"";
            let headers = format!(
                "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{total}\r\nContent-Length: 0\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n"
            );
            stream.write_all(headers.as_bytes())?;
            stream.write_all(body)?;
            return Ok(());
        }
        ("206 Partial Content", rs, end)
    } else {
        ("200 OK", 0u64, total.saturating_sub(1))
    };

    let len = end.saturating_sub(start).saturating_add(1);
    file.seek(SeekFrom::Start(start))?;

    let headers = if status.starts_with("206") {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {len}\r\nContent-Range: bytes {start}-{end}/{total}\r\nAccept-Ranges: bytes\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n"
        )
    } else {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {len}\r\nAccept-Ranges: bytes\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n"
        )
    };
    stream.write_all(headers.as_bytes())?;

    let mut remaining = len;
    let mut chunk = [0u8; 64 * 1024];
    while remaining > 0 {
        let to_read = remaining.min(chunk.len() as u64) as usize;
        let n = file.read(&mut chunk[..to_read])?;
        if n == 0 {
            break;
        }
        stream.write_all(&chunk[..n])?;
        remaining = remaining.saturating_sub(n as u64);
    }
    Ok(())
}

fn write_response(
    stream: &mut TcpStream,
    code: u16,
    mime: &str,
    body: &[u8],
    extra: Option<&str>,
) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Server Error",
        _ => "Error",
    };
    let extra = extra.unwrap_or("");
    let headers = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n{extra}\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes())?;
    stream.write_all(body)?;
    Ok(())
}
