use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    /// Display name shown in the UI. Replaced by the custom display name from
    /// settings.json once the user renames the profile in this app.
    pub name: String,
    /// The profile's `Name` exactly as it appears in `profiles.ini`.
    ///
    /// Zen resolves a launch by this value (`-P <zen_name>`), so it must be
    /// used when launching. `name` may have been renamed in-app and no longer
    /// matches anything in the browser's own configuration.
    pub zen_name: String,
    pub path: String,
    pub is_default: bool,
    pub avatar: Option<String>,
}
