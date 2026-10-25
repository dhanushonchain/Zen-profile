use crate::models::profile::Profile;
use crate::platform;
use crate::services::avatar_service;
use ini::Ini;
use std::path::Path;

/// Reads every Zen profile and overlays the custom names / avatars configured
/// in settings.json.
///
/// Returns `Err` only when `profiles.ini` exists but cannot be read or parsed.
/// A file that simply does not exist yet is *not* an error: Zen creates it on
/// its first run, so a fresh install legitimately has zero profiles.
pub fn get_profiles() -> Result<Vec<Profile>, String> {
    let mut profiles = load_profiles_from(&platform::profiles_ini_path())?;

    let settings = avatar_service::all_settings();
    apply_settings(&mut profiles, &settings);

    Ok(profiles)
}

/// Overlays custom display names and avatars from settings.json.
///
/// Only `name` and `avatar` are replaced — `zen_name` is deliberately left
/// untouched so that launching keeps addressing the real profile recorded in
/// `profiles.ini`, even after the user renames it in this app.
fn apply_settings(profiles: &mut [Profile], settings: &avatar_service::AppSettings) {
    for profile in profiles {
        if let Some(setting) = settings.profiles.get(&profile.id) {
            if let Some(name) = &setting.display_name {
                profile.name = name.clone();
            }
            if let Some(avatar) = &setting.avatar {
                profile.avatar = Some(avatar.clone());
            }
        }
    }
}

/// Loads and parses `profiles.ini` at `path`.
///
/// A missing file yields an empty list rather than an error, so the UI shows
/// its empty state instead of crashing. Any other failure — unreadable file,
/// malformed INI — is surfaced to the caller so the UI can report it rather
/// than silently showing an empty grid.
fn load_profiles_from(path: &Path) -> Result<Vec<Profile>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let ini = Ini::load_from_file(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    Ok(profiles_from_ini(&ini))
}

/// Extracts the `[ProfileN]` entries of an already-parsed document.
///
/// Pure — no filesystem access — so it can be unit-tested directly.
fn profiles_from_ini(ini: &Ini) -> Vec<Profile> {
    let mut profiles = Vec::new();

    for (section, properties) in ini {
        let Some(section) = section else {
            continue;
        };

        if !section.starts_with("Profile") {
            continue;
        }

        let name = properties.get("Name").unwrap_or("Unknown").to_string();

        let path = properties.get("Path").unwrap_or("").to_string();

        let is_default = properties.get("Default").map(|v| v == "1").unwrap_or(false);

        profiles.push(Profile {
            id: path.clone(),
            zen_name: name.clone(),
            name,
            path,
            is_default,
            avatar: None,
        });
    }

    profiles
}

pub fn launch_profile(profile_id: String) -> Result<(), String> {
    platform::launch_profile(&profile_id)
}

/// Creates a new Zen Browser profile by:
///   1. Validating the name.
///   2. Checking for duplicate names in profiles.ini.
///   3. Generating a random 8-char directory ID.
///   4. Creating the profile directory with prefs.js + times.json.
///   5. Appending a [ProfileN] entry to profiles.ini.
///
/// Zen initialises all other files (SQLite databases, extensions, etc.)
/// automatically the first time the profile is opened.
///
/// Any filesystem artefacts created before a failure are cleaned up so
/// no half-formed profile is ever left on disk.
pub fn create_profile(name: &str) -> Result<Profile, String> {
    let trimmed = validate_profile_name(name)?;

    // ── Duplicate check ───────────────────────────────────────────────────────
    let ini_path = platform::profiles_ini_path();

    // profiles.ini is absent until Zen has been launched once, so a fresh
    // install starts from an empty document — otherwise the very first
    // profile could never be created.
    let mut ini = if ini_path.exists() {
        Ini::load_from_file(&ini_path).map_err(|e| format!("Failed to read profiles.ini: {}", e))?
    } else {
        Ini::new()
    };

    for (section, props) in &ini {
        let Some(s) = section else { continue };
        if !s.starts_with("Profile") {
            continue;
        }
        if let Some(existing_name) = props.get("Name") {
            if existing_name.to_lowercase() == trimmed.to_lowercase() {
                return Err(format!(
                    "A profile named \"{}\" already exists.",
                    existing_name
                ));
            }
        }
    }

    // ── Build paths ───────────────────────────────────────────────────────────
    let profile_id = generate_profile_id();
    let dir_name = format!("{}.{}", profile_id, trimmed);
    let relative_path = format!("Profiles/{}", dir_name);
    let profile_dir = platform::profiles_directory().join(&dir_name);

    if profile_dir.exists() {
        // Astronomically unlikely with an 8-char random ID, but guard anyway.
        return Err("Generated a duplicate directory ID — please try again.".to_string());
    }

    // ── Create directory ──────────────────────────────────────────────────────
    std::fs::create_dir_all(&profile_dir)
        .map_err(|e| format!("Failed to create profile directory: {}", e))?;

    // Helper: clean up the new directory if any subsequent step fails.
    let cleanup = || {
        let _ = std::fs::remove_dir_all(&profile_dir);
    };

    // ── prefs.js ──────────────────────────────────────────────────────────────
    // Standard Mozilla empty-preferences header. Zen populates the rest on first launch.
    let prefs_content = "// Mozilla User Preferences\n\
        \n\
        // DO NOT EDIT THIS FILE.\n\
        //\n\
        // If you make changes to this file while the application is running,\n\
        // the changes will be overwritten when the application exits.\n\
        //\n\
        // To change a preference value, you can either:\n\
        // - modify it via the UI (e.g. via about:config in the browser); or\n\
        // - set it within a user.js file in your profile.\n\n";

    if let Err(e) = std::fs::write(profile_dir.join("prefs.js"), prefs_content) {
        cleanup();
        return Err(format!("Failed to write prefs.js: {}", e));
    }

    // ── times.json ────────────────────────────────────────────────────────────
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    if let Err(e) = std::fs::write(
        profile_dir.join("times.json"),
        format!("{{\"created\":{}}}\n", now_ms),
    ) {
        cleanup();
        return Err(format!("Failed to write times.json: {}", e));
    }

    // ── profiles.ini ──────────────────────────────────────────────────────────
    let next_n = find_next_profile_n(&ini);
    let section_name = format!("Profile{}", next_n);

    ini.with_section(Some(section_name))
        .set("Name", trimmed)
        .set("IsRelative", "1")
        .set("Path", &relative_path);

    if let Err(e) = ini.write_to_file(&ini_path) {
        cleanup();
        return Err(format!("Failed to update profiles.ini: {}", e));
    }

    Ok(Profile {
        id: relative_path.clone(),
        zen_name: trimmed.to_string(),
        name: trimmed.to_string(),
        path: relative_path,
        is_default: false,
        avatar: None,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Validates a user-supplied profile name, returning it trimmed on success.
///
/// Kept separate from `create_profile` so the rules are unit-testable without
/// touching the filesystem.
fn validate_profile_name(name: &str) -> Result<&str, String> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err("Profile name cannot be empty.".to_string());
    }
    if trimmed.len() > 100 {
        return Err("Profile name is too long (max 100 characters).".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\0') {
        return Err("Profile name contains invalid characters.".to_string());
    }

    Ok(trimmed)
}

/// Generates an 8-character lowercase alphanumeric profile ID using a
/// time-seeded linear congruential generator. No external crate required.
///
/// Collision probability: 1 in ~2.8 trillion — safe for local profile use.
fn generate_profile_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    // Mix millis + nanos for better entropy within the same second.
    let mut seed = (now.as_millis() as u64).wrapping_shl(17) ^ (now.subsec_nanos() as u64);

    let chars: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut result = String::with_capacity(8);

    for _ in 0..8 {
        // Knuth multiplicative hash step.
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        result.push(chars[(seed >> 33) as usize % chars.len()] as char);
    }

    result
}

/// Returns the next unused ProfileN number by scanning existing [ProfileN] sections.
fn find_next_profile_n(ini: &Ini) -> usize {
    let mut max: Option<usize> = None;

    for (section, _) in ini {
        let Some(s) = section else { continue };
        let Some(rest) = s.strip_prefix("Profile") else {
            continue;
        };
        let Ok(n) = rest.parse::<usize>() else {
            continue;
        };
        max = Some(match max {
            Some(m) => m.max(n),
            None => n,
        });
    }

    max.map_or(0, |m| m + 1)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Unique scratch path per test, so parallel test threads never collide.
    fn scratch_path(label: &str) -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "zen-profile-test-{}-{}-{}",
            std::process::id(),
            n,
            label
        ))
    }

    // ── Regression: the original code called `.unwrap()` here, which panicked
    //    and took the whole app down on any machine where Zen had not yet been
    //    launched (or on Linux, where the XDG directory may not exist yet).

    #[test]
    fn missing_profiles_ini_is_not_an_error() {
        let path = scratch_path("missing");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&path);

        let profiles = load_profiles_from(&path)
            .expect("a profiles.ini that does not exist yet must not be an error");

        assert!(profiles.is_empty());
    }

    #[test]
    fn unreadable_profiles_ini_is_reported_as_an_error() {
        // A directory where the file should be cannot be parsed as INI: the
        // open/read fails on both Unix and Windows, so we get Err rather than
        // a panic or a silent empty list.
        let path = scratch_path("directory");
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create scratch directory");

        let result = load_profiles_from(&path);
        let _ = std::fs::remove_dir_all(&path);

        assert!(
            result.is_err(),
            "an unreadable profiles.ini must surface an error, not an empty list"
        );
    }

    // ── profiles_from_ini ────────────────────────────────────────────────────

    #[test]
    fn parses_a_standard_profiles_ini() {
        let content = "\
[Profile0]
Name=zen
IsRelative=1
Path=Profiles/ab12cd34.zen
Default=1

[Profile1]
Name=work
IsRelative=1
Path=Profiles/ef56gh78.work
";
        let ini = Ini::load_from_str(content).unwrap();
        let profiles = profiles_from_ini(&ini);

        assert_eq!(profiles.len(), 2);

        assert_eq!(profiles[0].id, "Profiles/ab12cd34.zen");
        assert_eq!(profiles[0].name, "zen");
        assert_eq!(profiles[0].zen_name, "zen");
        assert_eq!(profiles[0].path, "Profiles/ab12cd34.zen");
        assert!(profiles[0].is_default);
        assert!(profiles[0].avatar.is_none());

        assert_eq!(profiles[1].id, "Profiles/ef56gh78.work");
        assert_eq!(profiles[1].name, "work");
        assert_eq!(profiles[1].zen_name, "work");
        assert!(!profiles[1].is_default);
    }

    // ── apply_settings ────────────────────────────────────────────────────────

    #[test]
    fn a_custom_display_name_does_not_change_zen_name() {
        let ini = Ini::load_from_str("[Profile0]\nName=zen\nPath=Profiles/abc.zen\n").unwrap();
        let mut profiles = profiles_from_ini(&ini);

        let mut settings = avatar_service::AppSettings::default();
        settings.profiles.insert(
            "Profiles/abc.zen".to_string(),
            avatar_service::ProfileSettings {
                display_name: Some("Work".to_string()),
                ..Default::default()
            },
        );

        apply_settings(&mut profiles, &settings);

        // The UI shows the custom name...
        assert_eq!(profiles[0].name, "Work");
        // ...but launching must still address the real profile recorded in
        // profiles.ini, or Zen would be asked to open a name that does not exist.
        assert_eq!(profiles[0].zen_name, "zen");
    }

    #[test]
    fn settings_only_apply_to_matching_profiles() {
        let ini = Ini::load_from_str("[Profile0]\nName=zen\nPath=Profiles/abc.zen\n").unwrap();
        let mut profiles = profiles_from_ini(&ini);

        let mut settings = avatar_service::AppSettings::default();
        settings.profiles.insert(
            "Profiles/someone-else".to_string(),
            avatar_service::ProfileSettings {
                display_name: Some("Other".to_string()),
                ..Default::default()
            },
        );

        apply_settings(&mut profiles, &settings);

        assert_eq!(profiles[0].name, "zen");
        assert_eq!(profiles[0].zen_name, "zen");
    }

    #[test]
    fn ignores_sections_that_are_not_profiles() {
        let content = "\
[General]
StartWithLastProfile=1

[Profile0]
Name=zen
Path=Profiles/abc.zen

[InstallABCDEF]
Default=Profiles/abc.zen
";
        let ini = Ini::load_from_str(content).unwrap();
        let profiles = profiles_from_ini(&ini);

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "zen");
    }

    #[test]
    fn tolerates_a_profile_entry_missing_optional_keys() {
        let ini = Ini::load_from_str("[Profile0]\nIsRelative=1\n").unwrap();
        let profiles = profiles_from_ini(&ini);

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "Unknown");
        assert_eq!(profiles[0].zen_name, "Unknown");
        assert_eq!(profiles[0].path, "");
        assert!(!profiles[0].is_default);
    }

    #[test]
    fn an_empty_document_yields_no_profiles() {
        let ini = Ini::new();
        assert!(profiles_from_ini(&ini).is_empty());
    }

    // ── find_next_profile_n ──────────────────────────────────────────────────

    #[test]
    fn find_next_profile_n_starts_at_zero() {
        assert_eq!(find_next_profile_n(&Ini::new()), 0);
    }

    #[test]
    fn find_next_profile_n_continues_after_the_highest() {
        let content = "\
[Profile0]
Name=a
Path=Profiles/a

[Profile3]
Name=b
Path=Profiles/b

[InstallXYZ]
Default=Profiles/a
";
        let ini = Ini::load_from_str(content).unwrap();

        // Highest profile number is 3, so the next is 4. The [InstallXYZ]
        // section must not be mistaken for a profile entry.
        assert_eq!(find_next_profile_n(&ini), 4);
    }

    // ── validate_profile_name ────────────────────────────────────────────────

    #[test]
    fn validates_and_trims_profile_names() {
        assert_eq!(validate_profile_name("  work  ").unwrap(), "work");
        assert!(validate_profile_name(&"x".repeat(100)).is_ok());

        assert!(validate_profile_name("").is_err());
        assert!(validate_profile_name("   ").is_err());
        assert!(validate_profile_name("a/b").is_err());
        assert!(validate_profile_name("name\0null").is_err());
        assert!(validate_profile_name(&"x".repeat(101)).is_err());
    }

    // ── generate_profile_id ──────────────────────────────────────────────────

    #[test]
    fn generates_an_eight_character_lowercase_alphanumeric_id() {
        let id = generate_profile_id();

        assert_eq!(id.len(), 8);
        assert!(
            id.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            "unexpected characters in profile id: {id}"
        );
    }
}
