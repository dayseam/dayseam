//! User-adjustable application settings.
//!
//! [`Settings`] is the canonical shape the frontend reads and the Rust
//! core persists. [`SettingsPatch`] is the partial-update counterpart
//! used by the `settings_update` IPC command — every field is
//! optional, and only `Some` fields are applied. This keeps settings
//! round-trips explicit about *what* changed, so the backend never has
//! to guess whether a missing field means "leave alone" or "reset to
//! default".
//!
//! The shape is deliberately minimal for v0.1 — only the handful of
//! preferences the app actually needs before any connector lands. New
//! preferences are added by extending both structs (always via `Option`
//! on `SettingsPatch`) plus the migration in
//! [`Settings::with_patch`]. Every addition bumps
//! [`Settings::CONFIG_VERSION`] so a stored-settings rehydrator can
//! detect legacy shapes and migrate them forward.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Preferred visual theme. Mirrors the frontend `Theme` union — kept
/// server-side too so a hypothetical "reset to default" action has a
/// single source of truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

impl Default for ThemePreference {
    fn default() -> Self {
        Self::System
    }
}

/// User-adjustable application settings. Persisted by the `settings`
/// repo under the single key `"app"` and surfaced to the frontend by
/// [`crate`]'s consumers via IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Settings {
    /// Monotonic schema version. Written by the Rust side on every
    /// save so a future migration can detect legacy shapes.
    pub config_version: u32,
    /// Visual theme preference.
    pub theme: ThemePreference,
    /// When true, the log drawer shows `Debug`-level rows. Off by
    /// default — Phase 1 still captures them into SQLite, the toggle
    /// only affects visibility.
    pub verbose_logs: bool,
}

impl Settings {
    /// Current schema version of the persisted settings blob. Bump
    /// whenever a field is added or reshaped so a future migration can
    /// tell legacy rows from current ones.
    pub const CONFIG_VERSION: u32 = 1;

    /// Apply every `Some(_)` field of `patch`, leave `None` fields
    /// untouched, and stamp `config_version` back to
    /// [`Self::CONFIG_VERSION`] so the stored shape always reflects
    /// the current schema.
    #[must_use]
    pub fn with_patch(mut self, patch: SettingsPatch) -> Self {
        if let Some(theme) = patch.theme {
            self.theme = theme;
        }
        if let Some(verbose) = patch.verbose_logs {
            self.verbose_logs = verbose;
        }
        self.config_version = Self::CONFIG_VERSION;
        self
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            config_version: Self::CONFIG_VERSION,
            theme: ThemePreference::default(),
            verbose_logs: false,
        }
    }
}

/// Partial update shape for [`Settings`]. Every field is optional so
/// the frontend can send only what the user changed; omitted fields
/// are explicitly "leave alone", not "reset to default".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SettingsPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemePreference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbose_logs: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_system_theme_and_quiet_logs() {
        let s = Settings::default();
        assert_eq!(s.theme, ThemePreference::System);
        assert!(!s.verbose_logs);
        assert_eq!(s.config_version, Settings::CONFIG_VERSION);
    }

    #[test]
    fn patch_applies_only_provided_fields() {
        let base = Settings {
            config_version: Settings::CONFIG_VERSION,
            theme: ThemePreference::Light,
            verbose_logs: false,
        };
        let patched = base.clone().with_patch(SettingsPatch {
            theme: Some(ThemePreference::Dark),
            verbose_logs: None,
        });
        assert_eq!(patched.theme, ThemePreference::Dark);
        assert!(!patched.verbose_logs);
    }

    #[test]
    fn empty_patch_is_identity() {
        let base = Settings::default();
        let patched = base.clone().with_patch(SettingsPatch::default());
        assert_eq!(base, patched);
    }

    #[test]
    fn patch_always_stamps_current_config_version() {
        let stale = Settings {
            config_version: 0,
            theme: ThemePreference::System,
            verbose_logs: true,
        };
        let patched = stale.with_patch(SettingsPatch::default());
        assert_eq!(patched.config_version, Settings::CONFIG_VERSION);
    }
}
