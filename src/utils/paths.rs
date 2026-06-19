//! Filesystem paths and small generic helpers: title sanitization, the storage
//! root, locating bundled tool binaries, and directory size.

use std::path::{Path, PathBuf};

pub fn clean_title(title: &str) -> String {
    title.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

pub fn get_storage_root() -> PathBuf {
    let config = crate::config::AppConfig::load();
    PathBuf::from(&config.storage_path)
}

/// Locate a bundled tool binary (e.g. `ffmpeg.exe`, `ffprobe.exe`).
///
/// Production builds ship these *next to the executable* — the installer copies
/// `runtime\*` into `{app}` and the portable zip flattens `runtime/<tool>.exe`
/// to the package root. The dev tree instead keeps them in `bin\`. We therefore
/// search, in order: `<exe_dir>\bin`, `<exe_dir>`, `<cwd>\bin`, `<cwd>`, then
/// fall back to the bare name (PATH). Searching the exe/cwd root — not just
/// `bin\` — is what lets installed builds find the bundled copy instead of
/// silently relying on a system-PATH install (which may be absent, exactly the
/// failure that left ffprobe unavailable and broke cross-session offsets).
pub(crate) fn find_bundled_binary(file_name: &str) -> PathBuf {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.to_path_buf());
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    for root in &roots {
        let in_bin = root.join("bin").join(file_name);
        if in_bin.exists() { return in_bin; }
        let alongside = root.join(file_name);
        if alongside.exists() { return alongside; }
    }
    PathBuf::from(file_name.trim_end_matches(".exe"))
}

pub fn get_ffmpeg_path() -> PathBuf {
    find_bundled_binary("ffmpeg.exe")
}

pub fn get_dir_size(path: &Path) -> std::io::Result<u64> {
    let mut size = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                size += get_dir_size(&path)?;
            } else {
                size += entry.metadata()?.len();
            }
        }
    }
    Ok(size)
}

/// Resolve a game title to its on-disk recording directory under the storage
/// root, applying `clean_title` (except the literal `monitor` source).
pub(crate) fn game_dir_for(game_title: &str) -> PathBuf {
    let safe_title = if game_title == "monitor" { "monitor".to_string() } else { clean_title(game_title) };
    get_storage_root().join(safe_title)
}
