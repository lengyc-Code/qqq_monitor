use crate::{AlertRule, MAX_ALERT_RULES, Notice};
use directories::ProjectDirs;
use qqq_domain::{Hotkey, RemoteAuth, RemoteEndpoint, Shortcut};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Screen edge the window docks to as a compact status bar.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DockEdge {
    Top,
    Left,
    Right,
}

/// Interface language, saved as a BCP 47 tag.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Language {
    /// Simplified Chinese.
    #[serde(rename = "zh-CN")]
    Chinese,
    #[serde(rename = "en")]
    English,
    #[serde(rename = "ja")]
    Japanese,
}

impl Language {
    pub const ALL: [Self; 3] = [Self::Chinese, Self::English, Self::Japanese];

    /// BCP 47 tag used in the configuration file and on the command line.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Chinese => "zh-CN",
            Self::English => "en",
            Self::Japanese => "ja",
        }
    }

    /// First supported language among BCP 47 tags in order of preference, such as the
    /// system display languages; every Chinese variant counts as Chinese. English when
    /// none is supported.
    pub fn preferred<'a>(tags: impl IntoIterator<Item = &'a str>) -> Self {
        tags.into_iter()
            .find_map(
                |tag| match tag.split(['-', '_']).next()?.to_ascii_lowercase().as_str() {
                    "zh" => Some(Self::Chinese),
                    "en" => Some(Self::English),
                    "ja" => Some(Self::Japanese),
                    _ => None,
                },
            )
            .unwrap_or(Self::English)
    }
}

/// How the user signs in to a remote host.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RemoteAuthKind {
    /// A password kept in the system's credential store.
    Password,
    /// A private key file; an encrypted key's passphrase is kept in the credential store.
    Key,
    /// The keys of the running SSH agent.
    Agent,
}

/// Most remote hosts the configuration holds.
pub const MAX_REMOTE_HOSTS: usize = 64;
/// Numbers of screenshots the configuration can keep.
pub const SCREENSHOT_HISTORY: [usize; 4] = [10, 20, 50, 100];
/// Numbers of clipboard entries the configuration can keep.
pub const CLIPBOARD_HISTORY: [usize; 4] = [20, 50, 100, 200];
/// Smallest inner size of the note window, in points.
pub const NOTE_MIN_SIZE: [f32; 2] = [300.0, 220.0];

/// A Linux host monitored over SSH. Passwords and passphrases stay out of the file.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteHost {
    /// Stable identity: letters, digits, `-` and `_`, at most 32 characters.
    pub id: String,
    /// Display name; the address when empty.
    #[serde(default)]
    pub name: String,
    pub address: String,
    #[serde(default = "ssh_port")]
    pub port: u16,
    pub user: String,
    pub auth: RemoteAuthKind,
    /// Private key file for `auth = "key"`.
    #[serde(default)]
    pub key_file: Option<String>,
    /// Host key the user trusted, as an OpenSSH public key line.
    #[serde(default)]
    pub host_key: Option<String>,
}

fn ssh_port() -> u16 {
    22
}

impl RemoteHost {
    pub fn validate(&self) -> Result<(), Notice> {
        let token = |text: &str| !text.is_empty() && !text.chars().any(char::is_whitespace);
        let valid = !self.id.is_empty()
            && self.id.len() <= 32
            && self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            && token(&self.address)
            && self.port != 0
            && token(&self.user)
            && !self.name.chars().any(char::is_control)
            && (self.auth != RemoteAuthKind::Key
                || self
                    .key_file
                    .as_deref()
                    .is_some_and(|f| !f.trim().is_empty()))
            && self
                .host_key
                .as_deref()
                .is_none_or(|k| !k.trim().is_empty());
        if valid {
            Ok(())
        } else {
            Err(Notice::InvalidRemoteHost)
        }
    }

    /// Display name of the host.
    pub fn label(&self) -> &str {
        if self.name.trim().is_empty() {
            &self.address
        } else {
            &self.name
        }
    }

    pub fn endpoint(&self) -> RemoteEndpoint {
        RemoteEndpoint {
            id: self.id.clone(),
            address: self.address.clone(),
            port: self.port,
            user: self.user.clone(),
            auth: match self.auth {
                RemoteAuthKind::Password => RemoteAuth::Password,
                RemoteAuthKind::Key => RemoteAuth::Key(self.key_file.clone().unwrap_or_default()),
                RemoteAuthKind::Agent => RemoteAuth::Agent,
            },
            host_key: self.host_key.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    /// Interface language; `None` follows the system display language at each start.
    pub language: Option<Language>,
    pub sampling_ms: u64,
    pub history_secs: u64,
    pub animations: bool,
    pub selected_network: Option<String>,
    pub window_size: [f32; 2],
    pub window_position: Option<[f32; 2]>,
    /// Dock a window dragged against the top, left or right screen edge.
    pub edge_docking: bool,
    /// Hide a minimized window in the notification area.
    pub minimize_to_tray: bool,
    /// Start with only the notification-area icon when Windows starts the program at sign-in.
    pub autostart_to_tray: bool,
    /// Edge of the compact status bar; `None` shows the full window.
    pub dock: Option<DockEdge>,
    /// Outer position of the compact status bar, in points.
    pub dock_position: Option<[f32; 2]>,
    /// Count the frames of the application in the foreground.
    pub frame_monitoring: bool,
    /// Shortcut that starts a screenshot from any application, written like `Ctrl+Alt+A`.
    pub screenshot_hotkey: String,
    /// Register the screenshot shortcut with the system.
    pub screenshot_hotkey_enabled: bool,
    /// Most screenshots kept, one of [`SCREENSHOT_HISTORY`].
    pub screenshot_history: usize,
    /// Record what is copied to the clipboard.
    pub clipboard_manager: bool,
    /// Most clipboard entries kept, one of [`CLIPBOARD_HISTORY`].
    pub clipboard_history: usize,
    /// Shortcut that shows or hides the note window from any application.
    pub note_hotkey: String,
    /// Register the note shortcut with the system.
    pub note_hotkey_enabled: bool,
    /// Inner size of the note window, in points.
    pub note_size: [f32; 2],
    /// Outer position of the note window, in points; `None` places it beside the main
    /// window's work area edge.
    pub note_position: Option<[f32; 2]>,
    /// Linux hosts monitored over SSH, in the order the GUI lists them. Saved after the
    /// plain values, as an array of tables.
    pub remote_hosts: Vec<RemoteHost>,
    /// Alert rules, in the order the GUI lists them. Saved last, as an array of tables.
    pub alerts: Vec<AlertRule>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: None,
            sampling_ms: 1000,
            history_secs: 60,
            animations: true,
            selected_network: None,
            window_size: [1280.0, 800.0],
            window_position: None,
            edge_docking: true,
            minimize_to_tray: true,
            autostart_to_tray: true,
            dock: None,
            dock_position: None,
            frame_monitoring: false,
            screenshot_hotkey: Hotkey::SCREENSHOT.to_string(),
            screenshot_hotkey_enabled: true,
            screenshot_history: 20,
            clipboard_manager: false,
            clipboard_history: 50,
            note_hotkey: Hotkey::NOTE.to_string(),
            note_hotkey_enabled: true,
            note_size: [360.0, 440.0],
            note_position: None,
            remote_hosts: Vec::new(),
            alerts: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn validate(&self) -> Result<(), Notice> {
        if ![500, 1000, 2000].contains(&self.sampling_ms) {
            return Err(Notice::InvalidSampling);
        }
        if ![60, 300, 900].contains(&self.history_secs) {
            return Err(Notice::InvalidHistory);
        }
        if !self
            .window_size
            .iter()
            .all(|v| v.is_finite() && (400.0..=16384.0).contains(v))
        {
            return Err(Notice::InvalidWindowSize);
        }
        if !self
            .note_size
            .iter()
            .zip(NOTE_MIN_SIZE)
            .all(|(v, min)| v.is_finite() && (min..=16384.0).contains(v))
        {
            return Err(Notice::InvalidWindowSize);
        }
        if [self.window_position, self.dock_position, self.note_position]
            .iter()
            .flatten()
            .any(|p| !p.iter().all(|v| v.is_finite()))
        {
            return Err(Notice::InvalidWindowPosition);
        }
        if Shortcut::ALL.iter().any(|s| self.hotkey(*s).is_none()) {
            return Err(Notice::InvalidHotkey);
        }
        if !SCREENSHOT_HISTORY.contains(&self.screenshot_history)
            || !CLIPBOARD_HISTORY.contains(&self.clipboard_history)
        {
            return Err(Notice::InvalidRecordLimit);
        }
        if self.remote_hosts.len() > MAX_REMOTE_HOSTS {
            return Err(Notice::InvalidRemoteHost);
        }
        for (index, host) in self.remote_hosts.iter().enumerate() {
            host.validate()?;
            if self.remote_hosts[..index].iter().any(|h| h.id == host.id) {
                return Err(Notice::InvalidRemoteHost);
            }
        }
        if self.alerts.len() > MAX_ALERT_RULES {
            return Err(Notice::InvalidAlertRule);
        }
        for (index, rule) in self.alerts.iter().enumerate() {
            rule.validate()?;
            if self.alerts[..index].iter().any(|r| r.id == rule.id) {
                return Err(Notice::InvalidAlertRule);
            }
        }
        Ok(())
    }

    /// A shortcut, whether or not it is enabled; `None` when the saved text is not a valid
    /// shortcut.
    pub fn hotkey(&self, shortcut: Shortcut) -> Option<Hotkey> {
        Hotkey::parse(self.shortcut(shortcut).0)
    }

    /// Written form of a shortcut and whether it is registered with the system.
    pub fn shortcut(&self, shortcut: Shortcut) -> (&str, bool) {
        match shortcut {
            Shortcut::Screenshot => (&self.screenshot_hotkey, self.screenshot_hotkey_enabled),
            Shortcut::Note => (&self.note_hotkey, self.note_hotkey_enabled),
        }
    }

    /// The written form of a shortcut and its switch, for changing them.
    pub fn shortcut_mut(&mut self, shortcut: Shortcut) -> (&mut String, &mut bool) {
        match shortcut {
            Shortcut::Screenshot => (
                &mut self.screenshot_hotkey,
                &mut self.screenshot_hotkey_enabled,
            ),
            Shortcut::Note => (&mut self.note_hotkey, &mut self.note_hotkey_enabled),
        }
    }
}

pub struct ConfigStore {
    path: PathBuf,
    protected: bool,
}

pub struct LoadedConfig {
    pub config: AppConfig,
    pub warning: Option<Notice>,
    pub store: ConfigStore,
}

pub fn data_directory() -> PathBuf {
    ProjectDirs::from("", "", "qqq_monitor")
        .map(|d| d.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".qqq_monitor"))
}

impl ConfigStore {
    pub fn load_default() -> LoadedConfig {
        let path = ProjectDirs::from("", "", "qqq_monitor")
            .map(|d| d.config_dir().join("config.toml"))
            .unwrap_or_else(|| PathBuf::from(".qqq_monitor/config.toml"));
        Self::load(path)
    }
    pub fn load(path: PathBuf) -> LoadedConfig {
        let mut config = AppConfig::default();
        let mut warning = None;
        let mut protected = false;
        match fs::read_to_string(&path) {
            Ok(text) => match text.parse::<toml::Table>() {
                Ok(table) => {
                    // Decode each field independently so one invalid preference does not discard the others.
                    macro_rules! field {
                        ($name:ident, $ty:ty) => {
                            if let Some(value) = table.get(stringify!($name)) {
                                match value.clone().try_into::<$ty>() {
                                    Ok(value) => {
                                        let mut candidate = config.clone();
                                        candidate.$name = value;
                                        if candidate.validate().is_ok() {
                                            config = candidate;
                                        } else {
                                            warning = Some(Notice::InvalidValue);
                                        }
                                    }
                                    Err(_) => warning = Some(Notice::InvalidType),
                                }
                            }
                        };
                    }
                    field!(language, Option<Language>);
                    field!(sampling_ms, u64);
                    field!(history_secs, u64);
                    field!(animations, bool);
                    field!(selected_network, Option<String>);
                    field!(window_size, [f32; 2]);
                    field!(window_position, Option<[f32; 2]>);
                    field!(edge_docking, bool);
                    field!(minimize_to_tray, bool);
                    field!(autostart_to_tray, bool);
                    field!(dock, Option<DockEdge>);
                    field!(dock_position, Option<[f32; 2]>);
                    field!(frame_monitoring, bool);
                    field!(screenshot_hotkey, String);
                    field!(screenshot_hotkey_enabled, bool);
                    field!(screenshot_history, usize);
                    field!(clipboard_manager, bool);
                    field!(clipboard_history, usize);
                    field!(note_hotkey, String);
                    field!(note_hotkey_enabled, bool);
                    field!(note_size, [f32; 2]);
                    field!(note_position, Option<[f32; 2]>);
                    // Each entry recovers on its own, so one bad entry keeps the others.
                    macro_rules! entries {
                        ($name:ident, $ty:ty, $limit:expr) => {
                            if let Some(value) = table.get(stringify!($name)) {
                                match value.as_array() {
                                    Some(items) => {
                                        for item in items {
                                            match item.clone().try_into::<$ty>() {
                                                Ok(entry)
                                                    if entry.validate().is_ok()
                                                        && config.$name.len() < $limit
                                                        && !config
                                                            .$name
                                                            .iter()
                                                            .any(|e| e.id == entry.id) =>
                                                {
                                                    config.$name.push(entry)
                                                }
                                                Ok(_) => warning = Some(Notice::InvalidValue),
                                                Err(_) => warning = Some(Notice::InvalidType),
                                            }
                                        }
                                    }
                                    None => warning = Some(Notice::InvalidType),
                                }
                            }
                        };
                    }
                    entries!(remote_hosts, RemoteHost, MAX_REMOTE_HOSTS);
                    entries!(alerts, AlertRule, MAX_ALERT_RULES);
                }
                Err(_) => {
                    protected = true;
                    warning = Some(Notice::CorruptFile);
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                protected = true;
                warning = Some(Notice::Unreadable(e.to_string()));
            }
        }
        LoadedConfig {
            config,
            warning,
            store: Self { path, protected },
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn save(&mut self, config: &AppConfig, explicit: bool) -> Result<(), Notice> {
        config.validate()?;
        if self.protected && !explicit {
            return Err(Notice::CorruptKept);
        }
        let parent = self.path.parent().ok_or(Notice::InvalidDirectory)?;
        fs::create_dir_all(parent).map_err(|e| Notice::System(e.to_string()))?;
        let temporary = parent.join(format!("config-{}.tmp", std::process::id()));
        let text = toml::to_string_pretty(config).map_err(|e| Notice::System(e.to_string()))?;
        let result = (|| -> io::Result<()> {
            let mut file = fs::File::create(&temporary)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, &self.path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|e| Notice::System(e.to_string()))?;
        self.protected = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferred_language_is_the_first_supported_tag() {
        assert_eq!(Language::preferred(["zh-TW"]), Language::Chinese);
        assert_eq!(
            Language::preferred(["fr-FR", "ja-JP", "en-US"]),
            Language::Japanese
        );
        assert_eq!(Language::preferred(["EN_gb"]), Language::English);
        assert_eq!(Language::preferred(["fr-FR", "de"]), Language::English);
        assert_eq!(Language::preferred([]), Language::English);
        for language in Language::ALL {
            assert_eq!(Language::preferred([language.tag()]), language);
        }
    }
}
