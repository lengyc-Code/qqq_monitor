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
/// Most quick commands the configuration holds.
pub const MAX_QUICK_COMMANDS: usize = 32;
/// Most command lines of one quick command.
pub const MAX_COMMAND_LINES: usize = 32;
/// Most UTF-8 bytes the command lines of one quick command take together, which keeps
/// every PowerShell command line within the system's limit.
pub const MAX_COMMAND_BYTES: usize = 8000;

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

/// How a quick command runs its command lines.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CommandSessions {
    /// One PowerShell session runs the lines in order, so later lines see the variables and
    /// location the earlier ones set.
    #[default]
    Shared,
    /// Each line starts in a PowerShell session of its own, all at once.
    Separate,
}

/// PowerShell command lines run together by a shortcut or from the tools page.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuickCommand {
    /// Stable identity: letters, digits, `-` and `_`, at most 32 characters.
    pub id: String,
    /// Display name, also the title of the session windows.
    pub name: String,
    /// One PowerShell command per entry, without line breaks.
    pub commands: Vec<String>,
    #[serde(default)]
    pub sessions: CommandSessions,
    /// Show each session in a console window that stays open after the commands end;
    /// otherwise the sessions run without a window and end with their commands.
    #[serde(default = "enabled")]
    pub window: bool,
    /// Working directory of the sessions; empty starts them in the user's profile folder.
    #[serde(default)]
    pub directory: String,
    /// Shortcut written like `Ctrl+Alt+1`; empty when the command has none.
    #[serde(default)]
    pub hotkey: String,
    /// Register the shortcut with the system.
    #[serde(default = "enabled")]
    pub hotkey_enabled: bool,
}

fn enabled() -> bool {
    true
}

/// A PowerShell session a quick command starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSession {
    /// Title of the session's window.
    pub title: String,
    /// The commands, one per line.
    pub script: String,
}

impl QuickCommand {
    pub fn validate(&self) -> Result<(), Notice> {
        let line = |text: &str| {
            !text.trim().is_empty() && !text.chars().any(|c| c.is_control() && c != '\t')
        };
        let valid = !self.id.is_empty()
            && self.id.len() <= 32
            && self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            && !self.name.trim().is_empty()
            && self.name.chars().count() <= 64
            && !self.name.chars().any(char::is_control)
            && !self.commands.is_empty()
            && self.commands.len() <= MAX_COMMAND_LINES
            && self.commands.iter().all(|c| line(c))
            && self.commands.iter().map(String::len).sum::<usize>() <= MAX_COMMAND_BYTES
            && !self.directory.chars().any(char::is_control)
            && (self.hotkey.is_empty() || Hotkey::parse(&self.hotkey).is_some());
        if valid {
            Ok(())
        } else {
            Err(Notice::InvalidQuickCommand)
        }
    }

    /// The sessions a run starts: one with every line, or one per line.
    pub fn sessions(&self) -> Vec<CommandSession> {
        match self.sessions {
            CommandSessions::Shared => vec![CommandSession {
                title: self.name.clone(),
                script: self.commands.join("\n"),
            }],
            CommandSessions::Separate => self
                .commands
                .iter()
                .map(|command| CommandSession {
                    title: format!("{} · {}", self.name, command.trim()),
                    script: command.clone(),
                })
                .collect(),
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
    /// Play a sound when a quick command runs.
    pub command_sound: bool,
    /// Linux hosts monitored over SSH, in the order the GUI lists them. Saved after the
    /// plain values, as an array of tables.
    pub remote_hosts: Vec<RemoteHost>,
    /// Alert rules, in the order the GUI lists them. Saved after the plain values, as an
    /// array of tables.
    pub alerts: Vec<AlertRule>,
    /// Quick commands, in the order the GUI lists them. Saved last, as an array of tables.
    pub quick_commands: Vec<QuickCommand>,
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
            command_sound: true,
            remote_hosts: Vec::new(),
            alerts: Vec::new(),
            quick_commands: Vec::new(),
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
        if Shortcut::BUILT_IN.iter().any(|s| self.hotkey(*s).is_none()) {
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
        if self.quick_commands.len() > MAX_QUICK_COMMANDS {
            return Err(Notice::InvalidQuickCommand);
        }
        for (index, command) in self.quick_commands.iter().enumerate() {
            command.validate()?;
            if self.quick_commands[..index]
                .iter()
                .any(|c| c.id == command.id)
            {
                return Err(Notice::InvalidQuickCommand);
            }
        }
        Ok(())
    }

    /// Every shortcut: the built-in ones, then one for each quick command.
    pub fn shortcuts(&self) -> impl Iterator<Item = Shortcut> + use<> {
        Shortcut::BUILT_IN
            .into_iter()
            .chain((0..self.quick_commands.len()).map(Shortcut::Command))
    }

    /// A shortcut, whether or not it is enabled; `None` when the saved text is not a valid
    /// shortcut, or a quick command has none.
    pub fn hotkey(&self, shortcut: Shortcut) -> Option<Hotkey> {
        Hotkey::parse(self.shortcut(shortcut).0)
    }

    /// Written form of a shortcut and whether it is registered with the system; empty and
    /// off for a quick command beyond the list.
    pub fn shortcut(&self, shortcut: Shortcut) -> (&str, bool) {
        match shortcut {
            Shortcut::Screenshot => (&self.screenshot_hotkey, self.screenshot_hotkey_enabled),
            Shortcut::Note => (&self.note_hotkey, self.note_hotkey_enabled),
            Shortcut::Command(index) => self
                .quick_commands
                .get(index)
                .map_or(("", false), |c| (&c.hotkey, c.hotkey_enabled)),
        }
    }

    /// The written form of a shortcut and its switch, for changing them; `None` for a quick
    /// command beyond the list.
    pub fn shortcut_mut(&mut self, shortcut: Shortcut) -> Option<(&mut String, &mut bool)> {
        match shortcut {
            Shortcut::Screenshot => Some((
                &mut self.screenshot_hotkey,
                &mut self.screenshot_hotkey_enabled,
            )),
            Shortcut::Note => Some((&mut self.note_hotkey, &mut self.note_hotkey_enabled)),
            Shortcut::Command(index) => self
                .quick_commands
                .get_mut(index)
                .map(|c| (&mut c.hotkey, &mut c.hotkey_enabled)),
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
                    field!(command_sound, bool);
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
                    entries!(quick_commands, QuickCommand, MAX_QUICK_COMMANDS);
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

    fn quick_command(commands: &[&str]) -> QuickCommand {
        QuickCommand {
            id: "dev".into(),
            name: "Dev".into(),
            commands: commands.iter().map(|c| (*c).into()).collect(),
            sessions: CommandSessions::Shared,
            window: true,
            directory: String::new(),
            hotkey: String::new(),
            hotkey_enabled: true,
        }
    }

    #[test]
    fn quick_commands_share_one_session_or_start_one_each() {
        let mut command = quick_command(&["cd C:\\code", "  cargo run"]);
        assert_eq!(
            command.sessions(),
            [CommandSession {
                title: "Dev".into(),
                script: "cd C:\\code\n  cargo run".into(),
            }]
        );
        command.sessions = CommandSessions::Separate;
        assert_eq!(
            command.sessions(),
            [
                CommandSession {
                    title: "Dev · cd C:\\code".into(),
                    script: "cd C:\\code".into(),
                },
                CommandSession {
                    title: "Dev · cargo run".into(),
                    script: "  cargo run".into(),
                },
            ]
        );
    }

    #[test]
    fn quick_commands_fit_one_command_line_each() {
        assert_eq!(quick_command(&["dir"]).validate(), Ok(()));
        assert_eq!(quick_command(&["\tdir"]).validate(), Ok(()));
        let long = "x".repeat(MAX_COMMAND_BYTES / 2);
        assert_eq!(quick_command(&[&long, &long]).validate(), Ok(()));
        let lines = vec!["dir"; MAX_COMMAND_LINES];
        assert_eq!(quick_command(&lines).validate(), Ok(()));
        let invalid = [
            quick_command(&[]),
            quick_command(&[" "]),
            quick_command(&["dir\ncls"]),
            quick_command(&[&long, &long, "x"]),
            quick_command(&vec!["dir"; MAX_COMMAND_LINES + 1]),
            QuickCommand {
                name: " ".into(),
                ..quick_command(&["dir"])
            },
            QuickCommand {
                id: "a b".into(),
                ..quick_command(&["dir"])
            },
            QuickCommand {
                hotkey: "Shift+A".into(),
                ..quick_command(&["dir"])
            },
        ];
        for command in invalid {
            assert_eq!(
                command.validate(),
                Err(Notice::InvalidQuickCommand),
                "{command:?}"
            );
        }
    }
}
