use crate::services::app_config_service::{self, AppConfig, Theme, VersionInfo};

// ── App Config ────────────────────────────────────────────────────────────────

/// Returns the current application configuration (theme, launch_at_login).
#[tauri::command]
pub fn get_app_config() -> AppConfig {
    app_config_service::get_config()
}

/// Persists a new theme preference. Accepts "system", "light", or "dark".
#[tauri::command]
pub fn set_theme(theme: String) -> Result<(), String> {
    let mut config = app_config_service::read_config();
    config.theme = match theme.to_lowercase().as_str() {
        "system" => Theme::System,
        "light" => Theme::Light,
        _ => Theme::Dark,
    };
    app_config_service::save_config(&config)
}

// ── Launch at Login ───────────────────────────────────────────────────────────

/// Enables or disables macOS launch at login via the autostart plugin,
/// then persists the new value to app_config.json.
#[tauri::command]
pub fn set_launch_at_login(enabled: bool, app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;

    if enabled {
        app.autolaunch().enable().map_err(|e| e.to_string())?;
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())?;
    }

    // Persist to app_config.json so we can show the correct state on next launch.
    let mut config = app_config_service::read_config();
    config.launch_at_login = enabled;
    app_config_service::save_config(&config)
}

/// Reads the current launch-at-login state directly from the OS via the autostart plugin.
/// This is the authoritative source — the OS state, not our cached config value.
#[tauri::command]
pub fn get_launch_at_login(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;

    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

// ── Zen Browser ───────────────────────────────────────────────────────────────

/// Opens a native file picker for the Zen Browser executable (a `.app` bundle
/// on macOS, an executable elsewhere), validates it, and stores it as the
/// custom launch path.
///
/// Returns `Ok(None)` when the user cancels the picker, so the caller can
/// leave the current setting untouched.
///
/// Uses `pick_file(callback)` rather than `blocking_pick_file()` because the
/// blocking variant dispatches NSOpenPanel.runModal() via dispatch_sync, which
/// conflicts with WRY/tao's main-thread event loop on macOS.
#[tauri::command]
pub async fn pick_zen_path(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::{DialogExt, FilePath};

    let (tx, rx) = tokio::sync::oneshot::channel::<Option<FilePath>>();

    app.dialog().file().pick_file(move |path| {
        let _ = tx.send(path);
    });

    let picked = rx
        .await
        .map_err(|_| "Dialog was dropped unexpectedly".to_string())?;

    let Some(file_path) = picked else {
        return Ok(None); // user cancelled
    };

    let FilePath::Path(path) = file_path else {
        return Err("Unsupported file path type".into());
    };

    let path_str = path.to_string_lossy().to_string();
    app_config_service::set_zen_path(Some(&path_str))?;
    Ok(Some(path_str))
}

/// Clears the custom Zen Browser path, restoring automatic detection.
#[tauri::command]
pub fn clear_zen_path() -> Result<(), String> {
    app_config_service::set_zen_path(None)
}

// ── About ─────────────────────────────────────────────────────────────────────

/// Returns static version and platform information for the Settings → About section.
#[tauri::command]
pub fn get_version_info(app: tauri::AppHandle) -> VersionInfo {
    app_config_service::get_version_info(&app)
}

// ── Storage ───────────────────────────────────────────────────────────────────

/// Opens the zen-profile settings directory in the OS file manager
/// (Finder / Explorer / the default file manager on Linux).
/// Creates the directory first if it does not yet exist.
#[tauri::command]
pub fn open_settings_folder(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;

    let dir = app_config_service::get_settings_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}
