use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub signaling_url: String,
    pub language: String,
    pub theme: String,
    pub start_at_login: bool,
    pub minimize_to_tray: bool,
    pub auto_update: bool,
    pub unattended: bool,
    pub require_confirm: bool,
    pub allow_clipboard: bool,
    pub allow_file_transfer: bool,
    pub lock_after_session: bool,
    pub p2p_preferred: bool,
    pub hardware_encode: bool,
    pub quality: String,
    pub fps: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            signaling_url: default_signaling_url(),
            language: "system".into(),
            theme: "system".into(),
            start_at_login: false,
            minimize_to_tray: true,
            auto_update: true,
            unattended: false,
            require_confirm: true,
            allow_clipboard: true,
            allow_file_transfer: true,
            lock_after_session: false,
            p2p_preferred: true,
            hardware_encode: true,
            quality: "balanced".into(),
            fps: 60,
        }
    }
}

impl AppSettings {
    pub fn load(dir: &Path) -> Result<Self> {
        let path = settings_path(dir);
        if !path.exists() {
            let settings = Self::default();
            settings.save(dir)?;
            return Ok(settings);
        }
        let raw = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(settings_path(dir), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

pub fn default_signaling_url() -> String {
    std::env::var("REMOTEX_SIGNALING")
        .unwrap_or_else(|_| "ws://127.0.0.1:7829/ws".into())
}

fn settings_path(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}
