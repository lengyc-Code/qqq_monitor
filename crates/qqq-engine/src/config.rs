use crate::Notice;
use directories::ProjectDirs;
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
    /// Edge of the compact status bar; `None` shows the full window.
    pub dock: Option<DockEdge>,
    /// Outer position of the compact status bar, in points.
    pub dock_position: Option<[f32; 2]>,
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
            dock: None,
            dock_position: None,
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
        if [self.window_position, self.dock_position]
            .iter()
            .flatten()
            .any(|p| !p.iter().all(|v| v.is_finite()))
        {
            return Err(Notice::InvalidWindowPosition);
        }
        Ok(())
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
                    field!(dock, Option<DockEdge>);
                    field!(dock_position, Option<[f32; 2]>);
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
