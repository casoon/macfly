use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::MacflyError;
use crate::plan_text::Lang;

/// Single Point of Truth: `~/.config/macfly/config.json` (oder `$MACFLY_CONFIG`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    /// Sprache der Ausgaben: `en` (Standard) oder `de`.
    #[serde(default)]
    pub language: Lang,
    #[serde(default = "default_profile_name")]
    pub default_profile: String,
    #[serde(default = "default_timeout_secs")]
    pub script_timeout_secs: u64,
    #[serde(default = "default_profiles")]
    pub profiles: BTreeMap<String, ProfileConfig>,
}

/// Vorkonfigurierte Sicherheitsstufe: legt fest, welche Änderungspläne ohne menschliche Freigabe laufen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Jede Änderung braucht eine menschliche Freigabe.
    Strict,
    /// Risikoarme Anlege-Aktionen (bis 5 Operationen) laufen automatisch.
    #[default]
    Balanced,
    /// Zusätzlich Verschieben, Anhängen und Löschen leerer Ordner automatisch.
    Open,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Strict, Preset::Balanced, Preset::Open];

    pub fn name(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Balanced => "balanced",
            Self::Open => "open",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|p| p.name().eq_ignore_ascii_case(name.trim()))
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Strict => {
                "Jede Änderung braucht deine Freigabe im Terminal. Für sensible Postfächer und Firmenrechner."
            }
            Self::Balanced => {
                "Notizen, Ordner, Mail-Entwürfe und Erinnerungen anlegen läuft automatisch (bis 5 Operationen). Verschieben, Anhängen, Löschen und Kurzbefehle brauchen deine Freigabe."
            }
            Self::Open => {
                "Zusätzlich Verschieben, Anhängen und Löschen leerer Ordner automatisch. Für Aufräum- und Migrations-Sessions. Kurzbefehle brauchen weiter deine Freigabe."
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    #[serde(default)]
    pub preset: Preset,
    #[serde(default)]
    pub notes: NotesScopeConfig,
    #[serde(default)]
    pub mail: MailScopeConfig,
    #[serde(default)]
    pub shortcuts: ShortcutsScopeConfig,
    #[serde(default)]
    pub dlp: DlpConfig,
    #[serde(default)]
    pub export: ExportScopeConfig,
}

/// Wohin Pläne Dateien schreiben dürfen (Foto-Export, später Mail-Anhänge).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportScopeConfig {
    /// Erlaubte Zielordner. Leer: nur unterhalb des Home-Ordners, nie in `~/Library`.
    #[serde(default)]
    pub allow_dirs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShortcutsScopeConfig {
    /// Explizite Allowlist freigegebener macOS-Kurzbefehle (Default: leer = alle gesperrt).
    #[serde(default)]
    pub allow: Vec<String>,
}

impl ShortcutsScopeConfig {
    pub fn is_allowed(&self, name: &str) -> bool {
        self.allow.iter().any(|a| a.eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotesScopeConfig {
    /// Wenn nicht leer, sind nur diese Accounts erlaubt.
    #[serde(default)]
    pub allow_accounts: Vec<String>,
    /// Wenn nicht leer, sind nur diese Ordnernamen erlaubt.
    #[serde(default)]
    pub allow_folders: Vec<String>,
    /// Ordner, die niemals gelesen/ausgegeben werden (z. B. "Passwörter", "Privat").
    #[serde(default = "default_deny_folders")]
    pub deny_folders: Vec<String>,
    /// Passwortgeschützte Apple-Notizen standardmäßig überspringen.
    #[serde(default = "default_true")]
    pub skip_password_protected: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MailScopeConfig {
    #[serde(default)]
    pub allow_accounts: Vec<String>,
    #[serde(default)]
    pub deny_mailboxes: Vec<String>,
    /// Single Point of Truth für Footer/Signaturen: direkt aus Apple Mail beziehen.
    #[serde(default)]
    pub signature: MailSignatureConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum MailSignatureConfig {
    /// Bezieht die Signatur direkt aus Apple Mail (kein doppelter Footer in der Config nötig).
    AppleMail {
        /// Optionaler Name der Signatur in Apple Mail; wenn None, wird die Standard-Signatur verwendet.
        #[serde(default)]
        name: Option<String>,
    },
    /// Optionaler expliziter Override, falls für KI-Antworten ein abweichender Footer gewünscht ist.
    Inline { text: String },
    /// Kein Footer anhängen.
    None,
}

impl Default for MailSignatureConfig {
    fn default() -> Self {
        Self::AppleMail { name: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_dlp_rules")]
    pub redact: Vec<String>,
    #[serde(default = "default_max_body_chars")]
    pub max_body_chars: usize,
}

impl Default for DlpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            redact: default_dlp_rules(),
            max_body_chars: default_max_body_chars(),
        }
    }
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            preset: Preset::default(),
            notes: NotesScopeConfig {
                allow_accounts: Vec::new(),
                allow_folders: Vec::new(),
                deny_folders: default_deny_folders(),
                skip_password_protected: true,
            },
            mail: MailScopeConfig::default(),
            shortcuts: ShortcutsScopeConfig::default(),
            dlp: DlpConfig::default(),
            export: ExportScopeConfig::default(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: default_schema_version(),
            language: Lang::default(),
            default_profile: default_profile_name(),
            script_timeout_secs: default_timeout_secs(),
            profiles: default_profiles(),
        }
    }
}

impl Config {
    pub fn default_path() -> PathBuf {
        if let Ok(env_path) = std::env::var("MACFLY_CONFIG") {
            return PathBuf::from(env_path);
        }
        if let Some(home) = dirs::home_dir() {
            return home.join(".config").join("macfly").join("config.json");
        }
        PathBuf::from("macfly.config.json")
    }

    pub fn load_or_default(path: Option<&Path>) -> Result<(Self, PathBuf, bool), MacflyError> {
        let cfg_path = path.map(PathBuf::from).unwrap_or_else(Self::default_path);
        if !cfg_path.exists() {
            return Ok((Self::default(), cfg_path, false));
        }
        let raw = fs::read_to_string(&cfg_path).map_err(|e| MacflyError::ConfigError {
            path: cfg_path.display().to_string(),
            detail: e.to_string(),
        })?;
        let parsed: Self = serde_json::from_str(&raw).map_err(|e| MacflyError::ConfigError {
            path: cfg_path.display().to_string(),
            detail: e.to_string(),
        })?;
        Ok((parsed, cfg_path, true))
    }

    pub fn write_default(
        path: Option<&Path>,
        force: bool,
        preset: Preset,
        language: Lang,
    ) -> Result<PathBuf, MacflyError> {
        let cfg_path = path.map(PathBuf::from).unwrap_or_else(Self::default_path);
        if cfg_path.exists() && !force {
            return Err(MacflyError::ConfigError {
                path: cfg_path.display().to_string(),
                detail: "Config existiert bereits (nutze --force zum Überschreiben)".to_string(),
            });
        }
        if let Some(parent) = cfg_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut cfg = Self {
            language,
            ..Self::default()
        };
        for profile in cfg.profiles.values_mut() {
            profile.preset = preset;
        }
        let json = serde_json::to_string_pretty(&cfg)?;
        fs::write(&cfg_path, format!("{json}\n"))?;
        Ok(cfg_path)
    }

    pub fn profile(&self, name: Option<&str>) -> Result<(&str, &ProfileConfig), MacflyError> {
        let target = name.unwrap_or(&self.default_profile);
        self.profiles
            .get_key_value(target)
            .map(|(k, p)| (k.as_str(), p))
            .ok_or_else(|| MacflyError::ConfigError {
                path: Self::default_path().display().to_string(),
                detail: format!("Unbekanntes Profil '{target}'"),
            })
    }
}

impl NotesScopeConfig {
    pub fn is_account_allowed(&self, account: &str) -> bool {
        if self.allow_accounts.is_empty() {
            return true;
        }
        self.allow_accounts
            .iter()
            .any(|a| a.eq_ignore_ascii_case(account))
    }

    pub fn is_folder_allowed(&self, folder: &str) -> bool {
        if self
            .deny_folders
            .iter()
            .any(|d| d.eq_ignore_ascii_case(folder))
        {
            return false;
        }
        if self.allow_folders.is_empty() {
            return true;
        }
        self.allow_folders
            .iter()
            .any(|a| a.eq_ignore_ascii_case(folder))
    }
}

impl MailScopeConfig {
    pub fn is_account_allowed(&self, account: &str) -> bool {
        if self.allow_accounts.is_empty() {
            return true;
        }
        self.allow_accounts
            .iter()
            .any(|a| a.eq_ignore_ascii_case(account))
    }

    pub fn is_mailbox_allowed(&self, mailbox: &str) -> bool {
        !self
            .deny_mailboxes
            .iter()
            .any(|d| d.eq_ignore_ascii_case(mailbox))
    }
}

impl ExportScopeConfig {
    /// Prüft einen Zielordner rein lexikalisch (`~` wird aufgelöst, `..` ist verboten).
    pub fn check_destination(&self, destination: &str) -> Result<PathBuf, MacflyError> {
        let home = dirs::home_dir();
        self.check_destination_in(destination, home.as_deref())
    }

    fn check_destination_in(
        &self,
        destination: &str,
        home: Option<&Path>,
    ) -> Result<PathBuf, MacflyError> {
        let deny = |detail: String| Err(MacflyError::ScopeDenied(detail));
        let raw = destination.trim();
        let path = if let Some(rest) = raw.strip_prefix("~/") {
            match home {
                Some(h) => h.join(rest),
                None => return deny("Home-Ordner nicht ermittelbar".to_string()),
            }
        } else {
            PathBuf::from(raw)
        };
        if !path.is_absolute() {
            return deny(format!("Zielordner '{raw}' muss ein absoluter Pfad sein"));
        }
        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return deny(format!("Zielordner '{raw}' darf kein '..' enthalten"));
        }
        if !self.allow_dirs.is_empty() {
            let allowed = self.allow_dirs.iter().any(|a| {
                let base = match a.strip_prefix("~/") {
                    Some(rest) => home.map(|h| h.join(rest)),
                    None => Some(PathBuf::from(a)),
                };
                base.is_some_and(|b| path.starts_with(b))
            });
            if !allowed {
                return deny(format!(
                    "Zielordner '{raw}' liegt außerhalb von `export.allow_dirs`"
                ));
            }
            return Ok(path);
        }
        let Some(home) = home else {
            return deny("Home-Ordner nicht ermittelbar".to_string());
        };
        if !path.starts_with(home) {
            return deny(format!(
                "Zielordner '{raw}' liegt außerhalb des Home-Ordners (erlaube ihn in `export.allow_dirs`)"
            ));
        }
        if path.starts_with(home.join("Library")) {
            return deny(format!("Zielordner '{raw}' liegt in ~/Library"));
        }
        Ok(path)
    }
}

pub fn data_dir() -> PathBuf {
    if let Ok(env_dir) = std::env::var("MACFLY_DATA_DIR") {
        return PathBuf::from(env_dir);
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".local").join("share").join("macfly");
    }
    PathBuf::from(".macfly-data")
}

fn default_schema_version() -> String {
    "1.0".to_string()
}

fn default_profile_name() -> String {
    "default".to_string()
}

fn default_timeout_secs() -> u64 {
    45
}

fn default_true() -> bool {
    true
}

fn default_max_body_chars() -> usize {
    12_000
}

fn default_deny_folders() -> Vec<String> {
    vec![
        "Passwörter".to_string(),
        "Passwords".to_string(),
        "Zuletzt gelöscht".to_string(),
        "Recently Deleted".to_string(),
    ]
}

fn default_dlp_rules() -> Vec<String> {
    vec![
        "private_key".to_string(),
        "api_key".to_string(),
        "iban".to_string(),
        "credit_card".to_string(),
        "otp_code".to_string(),
        "password".to_string(),
    ]
}

fn default_profiles() -> BTreeMap<String, ProfileConfig> {
    let mut map = BTreeMap::new();
    map.insert("default".to_string(), ProfileConfig::default());
    map
}

#[cfg(test)]
mod export_scope_tests {
    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("/Users/test")
    }

    #[test]
    fn default_scope_allows_home_but_not_library_or_outside() {
        let scope = ExportScopeConfig::default();
        let h = home();
        assert!(
            scope
                .check_destination_in("~/Pictures/Export", Some(&h))
                .is_ok()
        );
        assert!(
            scope
                .check_destination_in("/Users/test/Desktop/x", Some(&h))
                .is_ok()
        );
        assert!(
            scope
                .check_destination_in("~/Library/Preferences", Some(&h))
                .is_err()
        );
        assert!(
            scope
                .check_destination_in("/Volumes/Backup/x", Some(&h))
                .is_err()
        );
        assert!(scope.check_destination_in("/tmp/x", Some(&h)).is_err());
    }

    #[test]
    fn rejects_relative_and_parent_paths() {
        let scope = ExportScopeConfig::default();
        let h = home();
        assert!(scope.check_destination_in("Pictures/x", Some(&h)).is_err());
        assert!(
            scope
                .check_destination_in("~/Pictures/../Library/x", Some(&h))
                .is_err()
        );
    }

    #[test]
    fn allowlist_replaces_home_rule() {
        let scope = ExportScopeConfig {
            allow_dirs: vec![
                "/Volumes/Backup".to_string(),
                "~/Pictures/Export".to_string(),
            ],
        };
        let h = home();
        assert!(
            scope
                .check_destination_in("/Volumes/Backup/2026", Some(&h))
                .is_ok()
        );
        assert!(
            scope
                .check_destination_in("~/Pictures/Export/a", Some(&h))
                .is_ok()
        );
        assert!(scope.check_destination_in("~/Desktop", Some(&h)).is_err());
    }
}
