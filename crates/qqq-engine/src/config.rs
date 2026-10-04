use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub sampling_ms: u64,
    pub history_secs: u64,
    pub animations: bool,
    pub selected_network: Option<String>,
    pub window_size: [f32; 2],
    pub window_position: Option<[f32; 2]>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sampling_ms: 1000,
            history_secs: 60,
            animations: true,
            selected_network: None,
            window_size: [1280.0, 800.0],
            window_position: None,
        }
    }
}

impl AppConfig {
    pub fn validate(&self) -> Result<(), String> {
        if ![500, 1000, 2000].contains(&self.sampling_ms) {
            return Err("采样间隔必须为 0.5、1 或 2 秒".into());
        }
        if ![60, 300, 900].contains(&self.history_secs) {
            return Err("历史范围必须为 60、300 或 900 秒".into());
        }
        if !self
            .window_size
            .iter()
            .all(|v| v.is_finite() && (400.0..=16384.0).contains(v))
        {
            return Err("窗口尺寸无效".into());
        }
        if self
            .window_position
            .is_some_and(|p| !p.iter().all(|v| v.is_finite()))
        {
            return Err("窗口位置无效".into());
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
    pub warning: Option<String>,
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
                                    Ok(value) => { let mut candidate = config.clone(); candidate.$name = value; if candidate.validate().is_ok() { config = candidate; } else { warning = Some("部分配置无效，已恢复该项默认值".into()); } }
                                    Err(_) => warning = Some("部分配置类型无效，已恢复该项默认值".into()),
                                }
                            }
                        };
                    }
                    field!(sampling_ms, u64);
                    field!(history_secs, u64);
                    field!(animations, bool);
                    field!(selected_network, Option<String>);
                    field!(window_size, [f32; 2]);
                    field!(window_position, Option<[f32; 2]>);
                }
                Err(_) => {
                    protected = true;
                    warning =
                        Some("配置文件损坏，已使用默认值；原文件保留，点击保存设置后才替换".into());
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                protected = true;
                warning = Some(format!("无法读取配置：{e}"));
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
    pub fn save(&mut self, config: &AppConfig, explicit: bool) -> Result<(), String> {
        config.validate()?;
        if self.protected && !explicit {
            return Err("已保留损坏的配置，请在设置页面主动保存".into());
        }
        let parent = self.path.parent().ok_or("配置目录无效")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let temporary = parent.join(format!("config-{}.tmp", std::process::id()));
        let text = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
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
        result.map_err(|e| e.to_string())?;
        self.protected = false;
        Ok(())
    }
}
