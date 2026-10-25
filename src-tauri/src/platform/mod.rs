//! Platform abstraction layer.
//!
//! Every OS-specific path, executable lookup, and launch command lives in
//! `macos.rs` / `windows.rs` / `linux.rs`. The rest of the app only calls the
//! functions below and never checks `target_os` itself.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as os;
#[cfg(target_os = "macos")]
use macos as os;
#[cfg(target_os = "windows")]
use windows as os;

use std::path::PathBuf;

/// Zen Browser's own configuration directory (where `profiles.ini` lives).
pub fn config_directory() -> PathBuf {
    os::config_directory()
}

/// Path to Zen Browser's `profiles.ini`.
pub fn profiles_ini_path() -> PathBuf {
    config_directory().join("profiles.ini")
}

/// Directory containing individual profile folders.
pub fn profiles_directory() -> PathBuf {
    config_directory().join("Profiles")
}

/// Locates the Zen Browser executable.
///
/// A custom path set in Settings → Browser wins over the built-in per-OS
/// lookup; otherwise the normal auto-detection runs. Returns a descriptive
/// error (never panics) if it can't be found, so the caller/UI can offer a
/// manual path picker instead.
pub fn find_zen_browser() -> Result<PathBuf, String> {
    let configured = crate::services::app_config_service::get_config()
        .zen_path
        .map(PathBuf::from);

    resolve_zen_browser(configured, os::find_zen_browser())
}

/// Chooses between the user's custom path and OS auto-detection.
///
/// The custom path wins whenever it is present, *including* when it no longer
/// exists: silently falling back to auto-detection would hide a
/// misconfiguration from the exact person who set it, and could launch a
/// different Zen installation than the one they intended.
fn resolve_zen_browser(
    configured: Option<PathBuf>,
    detected: Result<PathBuf, String>,
) -> Result<PathBuf, String> {
    match configured {
        Some(path) if path.exists() => Ok(path),
        Some(path) => Err(format!(
            "Zen Browser was not found at the configured path {}. Update it in Settings \u{2192} Browser.",
            path.display()
        )),
        None => detected,
    }
}

/// Launches Zen Browser with the given profile.
pub fn launch_profile(profile_id: &str) -> Result<(), String> {
    let executable = find_zen_browser()?;
    os::launch_profile(&executable, profile_id)
}

/// zen-profile's own application data directory (settings.json, avatars/, app_config.json).
pub fn app_data_directory() -> PathBuf {
    os::app_data_directory()
}

/// The current user's home directory, for display purposes (e.g. substituting `~`).
pub fn home_directory() -> PathBuf {
    dirs::home_dir().expect("no home directory")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Guaranteed to exist on every OS.
    fn existing_path() -> PathBuf {
        std::env::temp_dir()
    }

    /// Guaranteed not to exist — its parent directory is never created.
    fn missing_path() -> PathBuf {
        std::env::temp_dir()
            .join("zen-profile-never-exists")
            .join("zen")
    }

    #[test]
    fn no_custom_path_falls_back_to_detection() {
        let detected = Ok(PathBuf::from("/usr/bin/zen"));
        assert_eq!(resolve_zen_browser(None, detected.clone()), detected);
    }

    #[test]
    fn no_custom_path_propagates_detection_failure() {
        let detected = Err("Zen Browser was not found.".to_string());
        assert_eq!(resolve_zen_browser(None, detected.clone()), detected);
    }

    #[test]
    fn a_custom_path_wins_over_detection() {
        let detected = Ok(PathBuf::from("/usr/bin/zen"));
        assert_eq!(
            resolve_zen_browser(Some(existing_path()), detected),
            Ok(existing_path())
        );
    }

    #[test]
    fn a_broken_custom_path_is_reported_not_ignored() {
        // Falling back to auto-detection here would silently launch some other
        // Zen install than the one the user deliberately chose.
        let detected = Ok(PathBuf::from("/usr/bin/zen"));
        let missing = missing_path();

        let err = resolve_zen_browser(Some(missing.clone()), detected).unwrap_err();

        assert!(err.contains("Settings"), "should point at Settings: {err}");
        assert!(
            err.contains(&missing.display().to_string()),
            "should quote the bad path: {err}"
        );
    }
}
