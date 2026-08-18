use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::Result;

/// Internet hub baked into the app so downloads work without editing a URL.
pub const PUBLIC_SIGNALING_URL: &str = "ws://23.226.134.88:7829/ws";
/// Second public hub (Singapore).
pub const LINE2_SIGNALING_URL: &str = "ws://8.222.218.229:7829/ws";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub signaling_url: String,
    #[serde(default = "default_signaling_line")]
    pub signaling_line: String,
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
        let mut settings = Self {
            signaling_url: default_signaling_url(),
            signaling_line: default_signaling_line(),
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
            quality: "high".into(),
            fps: 60,
        };
        settings.normalize();
        settings
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
        let mut settings: Self = serde_json::from_str(&raw)?;
        settings.normalize();
        let _ = settings.save(dir);
        Ok(settings)
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(settings_path(dir), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn normalize(&mut self) {
        if let Ok(url) = std::env::var("REMOTEX_SIGNALING") {
            if !url.trim().is_empty() {
                self.signaling_url = url;
                return;
            }
        }
        if self.signaling_line.trim() != "2" {
            self.signaling_line = "1".into();
        }
        self.signaling_url = url_for_line(&self.signaling_line).to_string();
    }
}

pub fn url_for_line(line: &str) -> &'static str {
    if line.trim() == "2" {
        LINE2_SIGNALING_URL
    } else {
        PUBLIC_SIGNALING_URL
    }
}

pub fn default_signaling_url() -> String {
    std::env::var("REMOTEX_SIGNALING")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| PUBLIC_SIGNALING_URL.to_string())
}

fn default_signaling_line() -> String {
    "1".into()
}

fn settings_path(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}
