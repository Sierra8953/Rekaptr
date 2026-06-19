//! Minimal TUS (resumable upload) client for pushing a clip's MP4 to Bunny
//! Stream. Mirrors what `tus-js-client` does in the web app's ShareClipButton:
//! a creation `POST` (with the Bunny auth headers + `Upload-Length`) followed
//! by chunked `PATCH`es. Blocking — call from a background thread.

use std::io::Read;
use std::path::Path;

use base64::Engine;

use super::api::UploadAuth;

// 16 MiB per PATCH — big enough to keep overhead low, small enough not to buffer
// a whole clip in memory.
const CHUNK_SIZE: usize = 16 * 1024 * 1024;
const TUS_VERSION: &str = "1.0.0";

/// Upload `file_path` to Bunny via TUS using the auth from `create_team_clip`.
/// `on_progress(sent, total)` is invoked after each chunk for the progress bar.
pub fn tus_upload(
    upload: &UploadAuth,
    file_path: &Path,
    title: &str,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let client = super::auth::blocking_client();

    let mut file = std::fs::File::open(file_path).map_err(|e| format!("open clip: {e}"))?;
    let total = file
        .metadata()
        .map_err(|e| format!("stat clip: {e}"))?
        .len();
    if total == 0 {
        return Err("Clip file is empty.".to_string());
    }

    // Bunny's TUS auth headers ride on every request.
    let with_auth = |req: reqwest::blocking::RequestBuilder| {
        req.header("AuthorizationSignature", &upload.signature)
            .header("AuthorizationExpire", upload.expire.to_string())
            .header("VideoId", &upload.guid)
            .header("LibraryId", upload.library_id.to_string())
    };

    // ── 1. Creation: announce the upload, get back its location.
    let b64 = base64::engine::general_purpose::STANDARD;
    let metadata = format!(
        "filetype {},title {}",
        b64.encode("video/mp4"),
        b64.encode(title)
    );
    let create = with_auth(
        client
            .post(&upload.endpoint)
            .header("Tus-Resumable", TUS_VERSION)
            .header("Upload-Length", total.to_string())
            .header("Upload-Metadata", metadata),
    )
    .send()
    .map_err(|e| format!("TUS create: {e}"))?;

    if !create.status().is_success() {
        return Err(format!("TUS create failed: {}", create.status()));
    }

    let location = create
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .ok_or("TUS create returned no Location")?;
    let upload_url = resolve_location(&upload.endpoint, location);

    // ── 2. Patch the file up in chunks.
    let mut offset: u64 = 0;
    let mut buf = vec![0u8; CHUNK_SIZE];
    while offset < total {
        let n = read_full(&mut file, &mut buf).map_err(|e| format!("read clip: {e}"))?;
        if n == 0 {
            break;
        }
        let chunk = buf[..n].to_vec();
        let resp = with_auth(
            client
                .patch(&upload_url)
                .header("Tus-Resumable", TUS_VERSION)
                .header("Upload-Offset", offset.to_string())
                .header("Content-Type", "application/offset+octet-stream")
                .body(chunk),
        )
        .send()
        .map_err(|e| format!("TUS upload: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("TUS upload failed: {}", resp.status()));
        }

        // Trust the server's Upload-Offset; fall back to our own count.
        offset = resp
            .headers()
            .get("Upload-Offset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(offset + n as u64);
        on_progress(offset, total);
    }

    Ok(())
}

/// Resolve a TUS `Location` (often relative, e.g. `/tusupload/<id>`) against the
/// creation endpoint's origin.
fn resolve_location(endpoint: &str, location: &str) -> String {
    if location.starts_with("http://") || location.starts_with("https://") {
        return location.to_string();
    }
    // Take scheme://host[:port] from the endpoint and append the path.
    if let Some(scheme_end) = endpoint.find("://") {
        let after = &endpoint[scheme_end + 3..];
        let host_end = after.find('/').map(|i| scheme_end + 3 + i).unwrap_or(endpoint.len());
        let origin = &endpoint[..host_end];
        if location.starts_with('/') {
            return format!("{origin}{location}");
        }
        return format!("{origin}/{location}");
    }
    location.to_string()
}

/// Read up to `buf.len()` bytes, looping until the buffer is full or EOF — a
/// single `read` may return fewer bytes than requested.
fn read_full(file: &mut std::fs::File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}
