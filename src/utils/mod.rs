//! Generic helpers, split by concern. This module is a thin façade: it declares
//! the focused submodules and re-exports their public API so callers keep using
//! the stable `crate::utils::*` paths.
//!
//! - [`paths`]      — storage root, title sanitization, bundled-binary lookup, dir size
//! - [`startup`]    — "start with Windows" registry toggle
//! - [`segments`]   — fragmented-MP4 segment filename/ffprobe primitives
//! - [`playlist`]   — HLS playlists, cross-session offset, clip marks/concat lists
//! - [`steam_assets`] — Steam appid resolution + icon/cover/artwork fetching
//! - [`clips_index`]  — recorded-source discovery, saved-clip library, source stats
//! - [`retention`]    — rolling-buffer retention + global size cap

mod clips_index;
mod paths;
mod playlist;
mod retention;
mod segments;
mod startup;
mod steam_assets;

pub use clips_index::{
    ensure_thumbnail, fetch_all_clips, scan_session_titles, source_stats, SourceStats,
};
pub use paths::{clean_title, get_dir_size, get_ffmpeg_path, get_storage_root};
pub use playlist::{
    build_clip_concat_list_from_marks, clip_duration_from_marks, compute_total_duration,
    fixup_eos_segments, generate_master_playlist, generate_session_playlist, mark_from_mpv_state,
};
pub use retention::start_buffer_cleanup_thread;
pub use startup::{is_startup_with_windows, set_startup_with_windows};
pub use steam_assets::{find_steam_cover, find_steam_icon, prefetch_artwork};

#[cfg(test)]
mod tests {
    use super::paths::clean_title;
    use super::playlist::compute_total_duration;
    use super::segments::{get_segment_duration, parse_segment_index};
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_clean_title_basic() {
        assert_eq!(clean_title("Counter-Strike 2"), "counterstrike2");
        assert_eq!(clean_title("Elden Ring"), "eldenring");
        assert_eq!(clean_title("DOOM (2016)"), "doom2016");
    }

    #[test]
    fn test_clean_title_special_chars() {
        assert_eq!(clean_title(""), "");
        assert_eq!(clean_title("!!!"), "");
        assert_eq!(clean_title("A"), "a");
        assert_eq!(clean_title("Test 123"), "test123");
    }

    #[test]
    fn test_parse_segment_index_valid() {
        assert_eq!(parse_segment_index("seg_0_6000ms.m4s"), Some(0));
        assert_eq!(parse_segment_index("seg_42_2000ms.m4s"), Some(42));
        assert_eq!(parse_segment_index("seg_100_1500ms.m4s"), Some(100));
    }

    #[test]
    fn test_parse_segment_index_no_duration() {
        // Segments that haven't been renamed yet (no _XXXms suffix)
        // Format is seg_INDEX_SESSIONID.m4s
        assert_eq!(parse_segment_index("seg_5_1234567890.m4s"), Some(5));
        assert_eq!(parse_segment_index("seg_0_1234567890.m4s"), Some(0));
    }

    #[test]
    fn test_parse_segment_index_invalid() {
        assert_eq!(parse_segment_index("not_a_segment.m4s"), None);
        assert_eq!(parse_segment_index("video.mp4"), None);
        assert_eq!(parse_segment_index(""), None);
        assert_eq!(parse_segment_index("seg_abc_100ms.m4s"), None);
    }

    #[test]
    fn test_get_segment_duration_from_filename() {
        let path = Path::new("seg_0_6000ms.m4s");
        assert_eq!(get_segment_duration(path), Some(6.0));

        let path = Path::new("seg_5_1500ms.m4s");
        assert_eq!(get_segment_duration(path), Some(1.5));

        let path = Path::new("seg_10_500ms.m4s");
        assert_eq!(get_segment_duration(path), Some(0.5));
    }

    #[test]
    fn test_get_segment_duration_no_duration_in_name() {
        let path = Path::new("seg_0.m4s");
        assert_eq!(get_segment_duration(path), None);

        let path = Path::new("video.mp4");
        assert_eq!(get_segment_duration(path), None);
    }

    #[test]
    fn test_compute_total_duration_empty_dir() {
        let dir = std::env::temp_dir().join("rekaptr_test_empty_dur");
        let _ = fs::create_dir_all(&dir);
        assert_eq!(compute_total_duration(&dir), 0.0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_compute_total_duration_with_unreadable_segments() {
        // compute_total_duration ffprobes the highest-indexed segment but falls
        // back to summing the `_XXXXms` filename durations when ffprobe can't
        // read a file (documented floor so the timeline keeps moving forward).
        // Fake-byte files can't be probed, so the filename sum wins: 6+6+3=15s.
        // We must not crash or hang walking back through the list.
        let dir = std::env::temp_dir().join("rekaptr_test_total_dur");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);

        fs::write(dir.join("seg_0_6000ms.m4s"), b"fake").unwrap();
        fs::write(dir.join("seg_1_6000ms.m4s"), b"fake").unwrap();
        fs::write(dir.join("seg_2_3000ms.m4s"), b"fake").unwrap();

        assert_eq!(compute_total_duration(&dir), 15.0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_compute_total_duration_nonexistent_dir() {
        let dir = Path::new("C:\\nonexistent_rekaptr_test_dir_12345");
        assert_eq!(compute_total_duration(dir), 0.0);
    }
}
