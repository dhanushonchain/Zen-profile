use crate::platform;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ── Types ─────────────────────────────────────────────────────────────────────

/// Color theme preference for the ZenProfile application UI.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    #[default]
    Dark,
    Light,
}

/// Application-level configuration.
///
/// Stored separately from profile metadata (`settings.json`) in `app_config.json`
/// so that app-level preferences are never mixed with per-profile data.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// UI color theme preference.
    pub theme: Theme,
    /// Whether ZenProfile should launch automatically at macOS login.
    pub launch_at_login: bool,

    /// User-chosen path to the Zen Browser executable (a `.app` bundle on
    /// macOS, an executable elsewhere).
    ///
    /// `None` means auto-detect from the built-in per-OS locations. Read by
    /// `platform::find_zen_browser`, which prefers it over auto-detection so
    /// non-standard installs can still be launched.
    ///
    /// `default` keeps `app_config.json` files written before this field
    /// existed readable; `skip_serializing_if` keeps the file clean while unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zen_path: Option<String>,

    /// Preserve any future unknown fields for forward compatibility.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Static build / platform information surfaced in the Settings → About section.
#[derive(Debug, Serialize)]
pub struct VersionInfo {
    pub app_name: String,
    pub app_version: String,
    pub tauri_version: String,
    pub platform: String,
    /// Human-readable path to the zen-profile settings directory (~ substituted).
    pub settings_dir: String,
}

// ── Paths ─────────────────────────────────────────────────────────────────────

fn app_config_path() -> PathBuf {
    platform::app_data_directory().join("app_config.json")
}

/// Returns the absolute path to the zen-profile settings directory.
/// Used by the open_settings_folder command.
pub fn get_settings_dir() -> PathBuf {
    platform::app_data_directory()
}

// ── Persistence ───────────────────────────────────────────────────────────────

pub fn read_config() -> AppConfig {
    let path = app_config_path();
    if !path.exists() {
        return AppConfig::default();
    }
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let path = app_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Returns the current application configuration.
pub fn get_config() -> AppConfig {
    read_config()
}

/// Validates a custom Zen Browser path and stores it (`None` clears it and
/// restores auto-detection).
///
/// Validation lives here rather than in the command layer so it is covered by
/// unit tests — a bad path would otherwise only surface later as a confusing
/// launch failure. Validation runs before any file I/O.
pub fn set_zen_path(path: Option<&str>) -> Result<(), String> {
    let validated = match path {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err("Zen Browser path cannot be empty.".to_string());
            }
            if !PathBuf::from(trimmed).exists() {
                return Err(format!(
                    "No file or application found at \"{}\". Choose a valid Zen Browser location.",
                    trimmed
                ));
            }
            Some(trimmed.to_string())
        }
        None => None,
    };

    let mut config = read_config();
    config.zen_path = validated;
    save_config(&config)
}

/// Returns build and platform information for the About section.
pub fn get_version_info(app: &tauri::AppHandle) -> VersionInfo {
    let pkg = app.package_info();

    // Substitute the real home directory with ~ for a cleaner display.
    let home = platform::home_directory().to_string_lossy().to_string();
    let full = platform::app_data_directory().to_string_lossy().to_string();
    let settings_dir = if full.starts_with(&home) {
        format!("~{}", &full[home.len()..])
    } else {
        full
    };

    VersionInfo {
        app_name: pkg.name.clone(),
        app_version: pkg.version.to_string(),
        tauri_version: "2".to_string(),
        platform: std::env::consts::OS.to_string(),
        settings_dir,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert_eq!(config.theme, Theme::Dark);
        assert!(!config.launch_at_login);
        assert!(config.extra.is_empty());
    }

    #[test]
    fn test_app_config_roundtrip() {
        let config = AppConfig {
            theme: Theme::System,
            launch_at_login: true,
            zen_path: None,
            extra: HashMap::new(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let restored: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.theme, Theme::System);
        assert!(restored.launch_at_login);
        assert_eq!(restored.zen_path, None);
    }

    #[test]
    fn test_theme_serialization() {
        assert_eq!(serde_json::to_string(&Theme::Dark).unwrap(), "\"dark\"");
        assert_eq!(serde_json::to_string(&Theme::Light).unwrap(), "\"light\"");
        assert_eq!(serde_json::to_string(&Theme::System).unwrap(), "\"system\"");
    }

    #[test]
    fn test_extra_fields_preserved() {
        let json = r#"{"theme":"dark","launch_at_login":false,"future_feature":true}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, Theme::Dark);
        assert_eq!(
            config.extra.get("future_feature").unwrap(),
            &serde_json::Value::Bool(true)
        );
    }

    // ── zen_path ──────────────────────────────────────────────────────────────

    #[test]
    fn test_existing_configs_without_zen_path_still_load() {
        // app_config.json written before zen_path existed must not fail to
        // deserialize — a lost config would reset the user's theme too.
        let json = r#"{"theme":"light","launch_at_login":true}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, Theme::Light);
        assert_eq!(config.zen_path, None);
    }

    #[test]
    fn test_zen_path_roundtrip() {
        let config = AppConfig {
            zen_path: Some("/opt/zen/zen".to_string()),
            ..AppConfig::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let restored: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.zen_path.as_deref(), Some("/opt/zen/zen"));
    }

    #[test]
    fn test_unset_zen_path_is_omitted_from_json() {
        let json = serde_json::to_string(&AppConfig::default()).unwrap();
        assert!(
            !json.contains("zen_path"),
            "unset zen_path should not be written to app_config.json: {json}"
        );
    }

    #[test]
    fn test_empty_zen_path_is_rejected() {
        let err = set_zen_path(Some("   ")).unwrap_err();
        assert!(err.contains("cannot be empty"), "unexpected error: {err}");
    }

    #[test]
    fn test_nonexistent_zen_path_is_rejected() {
        // A path beneath a directory this app never creates, so it cannot exist.
        let missing = std::env::temp_dir()
            .join("zen-profile-never-exists")
            .join("zen");
        let missing_str = missing.to_string_lossy().to_string();

        let err = set_zen_path(Some(&missing_str)).unwrap_err();
        assert!(
            err.contains(&missing_str),
            "error should quote the bad path: {err}"
        );
        assert!(
            err.contains("No file or application found"),
            "unexpected error: {err}"
        );
    }
}
