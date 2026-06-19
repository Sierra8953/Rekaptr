//! Steam game artwork & icons: resolve a title to an appid (catalog → memoized
//! storesearch), then fetch/cache icons (clienticon `.ico` via `appinfo.vdf`),
//! portrait covers and hero/logo art from the Steam CDN.

use std::path::PathBuf;

use winreg::enums::*;
use winreg::RegKey;

use super::paths::get_storage_root;

/// Strip zero-width / formatting / control characters and collapse whitespace.
/// Some game window titles are riddled with U+200B (ZWSP) and U+FEFF (BOM)
/// between nearly every letter (e.g. ARC Raiders). They're invisible, so the
/// display and the catalog's alphanumeric `normalize` cope — but Steam's
/// storesearch tokenizer returns nothing for such a term. Reduce it to clean,
/// human-readable words before searching.
fn sanitize_search_term(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .filter(|c| {
            !c.is_control()
                && !matches!(
                    *c,
                    '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{200E}' | '\u{200F}'
                        | '\u{2060}' | '\u{FEFF}' | '\u{00AD}'
                )
        })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn resolve_steam_app_id(game_title: &str) -> Option<String> {
    // 1. Bundled/cached catalog of popular games — instant, offline, no network.
    if let Some(appid) = crate::game_catalog::lookup_appid(game_title) {
        return Some(appid.to_string());
    }

    // 2. Per-process memo so a catalog miss is searched at most once per run
    //    (including negative results, so we don't re-hit the network on retries).
    static SEARCH_MEMO: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Option<String>>>,
    > = std::sync::OnceLock::new();
    let memo = SEARCH_MEMO.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(cached) = memo.lock().ok().and_then(|m| m.get(game_title).cloned()) {
        return cached;
    }

    // 3. Network fallback: Steam storesearch. Sanitize first — raw window titles
    //    can be peppered with invisible zero-width/BOM chars that make the search
    //    return nothing.
    let term = sanitize_search_term(game_title);
    let result = (|| {
        if term.is_empty() {
            return None;
        }
        let url = format!(
            "https://store.steampowered.com/api/storesearch/?term={}&l=english&cc=US",
            url::form_urlencoded::byte_serialize(term.as_bytes()).collect::<String>()
        );

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .user_agent("Rekaptr/1.0")
            .build()
            .ok()?;

        let json = client.get(&url).send().ok()?.json::<serde_json::Value>().ok()?;
        let id = json.get("items")?.as_array()?.first()?.get("id")?.as_i64()?;
        log::info!("[Utils] Found Steam AppID {} for '{}' via search API", id, term);
        Some(id.to_string())
    })();

    if result.is_none() {
        log::warn!(
            "[Utils] No Steam AppID for '{}' (not in catalog, storesearch returned nothing)",
            term
        );
    }

    if let Ok(mut m) = memo.lock() {
        m.insert(game_title.to_string(), result.clone());
    }
    result
}

fn resolve_steam_artwork(game_title: &str, cdn_filename: &str, cache_suffix: &str) -> Option<String> {
    let app_id = resolve_steam_app_id(game_title)?;

    let cache_dir = get_storage_root().join("Cache").join("Artwork");
    let _ = std::fs::create_dir_all(&cache_dir);

    // Check for cached file with any common image extension
    for ext in &["webp", "png", "jpg"] {
        let local_path = cache_dir.join(format!("{}_{}.{}", app_id, cache_suffix, ext));
        if local_path.exists() {
            if let Ok(meta) = local_path.metadata() {
                if meta.len() > 5000 {
                    let path_str = local_path.to_string_lossy().replace('\\', "/");
                    return Some(path_str);
                } else {
                    let _ = std::fs::remove_file(&local_path);
                }
            }
        }
    }

    let url = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/{}", app_id, cdn_filename);
    Some(url)
}

/// Local Steam install directory, from the registry. `None` if Steam isn't
/// installed / the key is missing.
fn steam_install_path() -> Option<PathBuf> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(k) = hkcu.open_subkey("Software\\Valve\\Steam") {
        if let Ok(p) = k.get_value::<String, _>("SteamPath") {
            let pb = PathBuf::from(p.replace('/', "\\"));
            if pb.exists() {
                return Some(pb);
            }
        }
    }
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for sub in ["SOFTWARE\\WOW6432Node\\Valve\\Steam", "SOFTWARE\\Valve\\Steam"] {
        if let Ok(k) = hklm.open_subkey(sub) {
            if let Ok(p) = k.get_value::<String, _>("InstallPath") {
                let pb = PathBuf::from(p);
                if pb.exists() {
                    return Some(pb);
                }
            }
        }
    }
    None
}

/// Square game icon for the sources list. Prefers the crisp 256px **clienticon**
/// (`steam/games/<hash>.ico`, the same asset the mockup used) and falls back to
/// the small librarycache `.jpg` (≈32px, blurry when scaled, but better than a
/// letter tile). `None` if Steam/the appid/all icons can't be found. Blocking
/// (appid resolution and the CDN fetch may hit the network) — call off the UI
/// thread.
///
/// The clienticon is keyed by a hash that is *not* derivable from the
/// librarycache filenames — it lives in `appinfo.vdf` (see
/// [`clienticon_hashes`]). With the hash we use the local `.ico` if present,
/// else download it from the community CDN, then cache its largest frame as PNG.
pub fn find_steam_icon(game_title: &str) -> Option<PathBuf> {
    let steam = steam_install_path()?;
    let app_id = resolve_steam_app_id(game_title)?;

    // Preferred: the crisp clienticon `.ico` (frames up to 256px).
    if let Some(hash) = clienticon_hashes().get(&app_id) {
        let local = steam.join("steam").join("games").join(format!("{}.ico", hash));
        let ico = if local.metadata().map(|m| m.len() > 100).unwrap_or(false) {
            Some(local)
        } else {
            download_clienticon_ico(&app_id, hash)
        };
        if let Some(ico) = ico {
            if let Some(png) = extract_ico_largest_frame(&ico, hash) {
                return Some(png);
            }
        }
    }

    // Fallback: the small librarycache icon `.jpg`. Legacy flat layout first
    // (`<appid>_icon.jpg`), then the current `librarycache/<appid>/<hash>.jpg` —
    // that folder holds the hero / logo / capsule (large) plus the icon, which
    // is the smallest image file (≈1–3 KB for a 32px image).
    let cache = steam.join("appcache").join("librarycache");
    let legacy = cache.join(format!("{}_icon.jpg", app_id));
    if legacy.metadata().map(|m| m.len() > 100).unwrap_or(false) {
        return Some(legacy);
    }
    let mut best: Option<(u64, PathBuf)> = None;
    if let Ok(rd) = std::fs::read_dir(cache.join(&app_id)) {
        for entry in rd.flatten() {
            let path = entry.path();
            let is_img = path
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| matches!(x.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png"))
                .unwrap_or(false);
            if !is_img {
                continue;
            }
            if let Ok(m) = entry.metadata() {
                let len = m.len();
                if len > 100 && best.as_ref().map_or(true, |(bl, _)| len < *bl) {
                    best = Some((len, path));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Map of `appid` → clienticon hash, parsed once from Steam's `appinfo.vdf` and
/// cached for the process. The clienticon hash names the crisp 256px `.ico`
/// (local `steam/games/<hash>.ico` or the community CDN); it can't be derived
/// from librarycache filenames. Empty if Steam isn't installed or the file is
/// in an unsupported format. Blocking on first call (reads the ~2 MB file).
fn clienticon_hashes() -> &'static std::collections::HashMap<String, String> {
    static MAP: std::sync::OnceLock<std::collections::HashMap<String, String>> =
        std::sync::OnceLock::new();
    MAP.get_or_init(|| parse_clienticon_hashes().unwrap_or_default())
}

/// Parse `appinfo.vdf` (current format, magic `0x07564429`). That format interns
/// every key name in a trailing string table and references it by u32 index in
/// each app's binary KV blob, so we find the string-table index of `clienticon`
/// and scan each app's blob for a string node (`0x01`) carrying that key — no
/// full KV-tree walk needed, since app-entry boundaries come from a size field.
fn parse_clienticon_hashes() -> Option<std::collections::HashMap<String, String>> {
    let steam = steam_install_path()?;
    let b = std::fs::read(steam.join("appcache").join("appinfo.vdf")).ok()?;
    if b.len() < 16 {
        return None;
    }
    let magic = u32::from_le_bytes(b[0..4].try_into().ok()?);
    if magic != 0x07564429 {
        // Older inline-key formats would need a different walk; fall back to jpgs.
        log::warn!("[Utils] appinfo.vdf magic {:#x} unsupported; using small icons", magic);
        return None;
    }
    let str_table_off = i64::from_le_bytes(b[8..16].try_into().ok()?) as usize;
    if str_table_off + 4 > b.len() {
        return None;
    }

    // String table: u32 count, then `count` NUL-terminated UTF-8 strings.
    let count = u32::from_le_bytes(b[str_table_off..str_table_off + 4].try_into().ok()?) as usize;
    let mut p = str_table_off + 4;
    let mut ci_idx: Option<u32> = None;
    for i in 0..count {
        let start = p;
        while p < b.len() && b[p] != 0 {
            p += 1;
        }
        if ci_idx.is_none() && &b[start..p] == b"clienticon" {
            ci_idx = Some(i as u32);
        }
        p += 1;
    }
    let idx_le = ci_idx?.to_le_bytes();

    // App entries: u32 appid (0 terminates), u32 size, then `size` bytes of fixed
    // header + KV blob. Scan each blob for `0x01 <clienticon-idx LE>` + value.
    let mut map = std::collections::HashMap::new();
    let mut pos = 16usize;
    while pos + 8 <= str_table_off {
        let appid = u32::from_le_bytes(b[pos..pos + 4].try_into().ok()?);
        pos += 4;
        if appid == 0 {
            break;
        }
        let size = u32::from_le_bytes(b[pos..pos + 4].try_into().ok()?) as usize;
        pos += 4;
        let end = (pos + size).min(b.len());
        let blob = &b[pos..end];
        let mut j = 0;
        while j + 5 < blob.len() {
            if blob[j] == 0x01 && blob.get(j + 1..j + 5) == Some(&idx_le[..]) {
                let vs = j + 5;
                let mut ve = vs;
                while ve < blob.len() && blob[ve] != 0 {
                    ve += 1;
                }
                if let Ok(hash) = std::str::from_utf8(&blob[vs..ve]) {
                    if !hash.is_empty() {
                        map.insert(appid.to_string(), hash.to_string());
                    }
                }
                break;
            }
            j += 1;
        }
        pos = end;
    }
    log::info!("[Utils] Parsed {} clienticon hashes from appinfo.vdf", map.len());
    Some(map)
}

/// Download a clienticon `.ico` from the Steam community CDN into the local icon
/// cache, for games whose `.ico` isn't on disk (e.g. not installed). Returns the
/// cached `.ico` path; the caller extracts a PNG frame. Blocking.
fn download_clienticon_ico(app_id: &str, hash: &str) -> Option<PathBuf> {
    let out_dir = get_storage_root().join("Cache").join("Icons");
    let out = out_dir.join(format!("{}.ico", hash));
    if out.metadata().map(|m| m.len() > 100).unwrap_or(false) {
        return Some(out);
    }
    let url = format!(
        "https://cdn.cloudflare.steamstatic.com/steamcommunity/public/images/apps/{}/{}.ico",
        app_id, hash
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Rekaptr/1.0")
        .build()
        .ok()?;
    let bytes = client.get(&url).send().ok()?.bytes().ok()?;
    if bytes.len() <= 100 {
        return None;
    }
    let _ = std::fs::create_dir_all(&out_dir);
    std::fs::write(&out, &bytes).ok()?;
    Some(out)
}

/// Decode an `.ico`, pick its largest frame, and write it as a PNG to the local
/// icon cache (keyed by `hash` so it's only done once). Returns the PNG path.
fn extract_ico_largest_frame(ico_path: &std::path::Path, hash: &str) -> Option<PathBuf> {
    let out_dir = get_storage_root().join("Cache").join("Icons");
    let out = out_dir.join(format!("{}.png", hash));
    if out.metadata().map(|m| m.len() > 100).unwrap_or(false) {
        return Some(out);
    }
    let file = std::fs::File::open(ico_path).ok()?;
    let dir = ico::IconDir::read(file).ok()?;
    let entry = dir.entries().iter().max_by_key(|e| e.width())?;
    let image = entry.decode().ok()?;
    let buf = image::RgbaImage::from_raw(image.width(), image.height(), image.rgba_data().to_vec())?;
    let _ = std::fs::create_dir_all(&out_dir);
    buf.save(&out).ok()?;
    Some(out)
}

/// Returns Steam's portrait library cover (2:3, 600x900) URL/path — the box-art
/// used for the per-game "folder" posters on the clips page.
pub fn find_steam_cover(game_title: &str) -> Option<String> {
    resolve_steam_artwork(game_title, "library_600x900.jpg", "cover")
}

/// Download one artwork asset to the local cache if not already present.
/// Returns the cached file path, or `None` if the appid can't be resolved or
/// the download is missing/too small. Blocking.
fn download_artwork_to_cache(game_title: &str, cdn_filename: &str, cache_suffix: &str) -> Option<std::path::PathBuf> {
    let app_id = resolve_steam_app_id(game_title)?;
    let cache_dir = get_storage_root().join("Cache").join("Artwork");
    let _ = std::fs::create_dir_all(&cache_dir);

    // Already cached (any common extension, non-trivial size)?
    for ext in &["webp", "png", "jpg"] {
        let p = cache_dir.join(format!("{}_{}.{}", app_id, cache_suffix, ext));
        if p.metadata().map(|m| m.len() > 5000).unwrap_or(false) {
            return Some(p);
        }
    }

    let url = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/{}", app_id, cdn_filename);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Rekaptr/1.0")
        .build()
        .ok()?;
    let bytes = client.get(&url).send().ok()?.bytes().ok()?;
    if bytes.len() <= 5000 {
        return None;
    }
    let ext = cdn_filename.rsplit('.').next().unwrap_or("jpg");
    let path = cache_dir.join(format!("{}_{}.{}", app_id, cache_suffix, ext));
    std::fs::write(&path, &bytes).ok()?;
    Some(path)
}

/// Warm the artwork cache for the given game titles on a background thread, so
/// their dashboard/clips cards render without an on-demand network fetch. Also
/// refreshes the popular-games catalog if it's stale. Titles that aren't real
/// games (empty, "monitor", "desktop") are skipped. Non-blocking.
pub fn prefetch_artwork(titles: Vec<String>) {
    let _ = std::thread::Builder::new()
        .name("rekaptr-artwork-prefetch".into())
        .spawn(move || {
            crate::game_catalog::refresh_if_stale();
            for title in titles {
                let t = title.trim();
                if t.is_empty() || t.eq_ignore_ascii_case("monitor") || t.eq_ignore_ascii_case("desktop") {
                    continue;
                }
                // Steam's pre-blurred hero drives the source cards; logo is the
                // transparent overlay on the dashboard video preview.
                let _ = download_artwork_to_cache(t, "library_hero_blur.jpg", "heroblur");
                let _ = download_artwork_to_cache(t, "logo.png", "logo");
                // Portrait cover (2:3) drives the per-game folder posters on the clips page.
                let _ = download_artwork_to_cache(t, "library_600x900.jpg", "cover");
            }
        });
}
