use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
    #[serde(default)]
    pub settings_rev: u32,
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

#[derive(Debug, Clone)]
pub struct LinePick {
    pub line: String,
    pub url: String,
    pub line1_rtt_ms: u32,
    pub line2_rtt_ms: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        let mut settings = Self {
            signaling_url: default_signaling_url(),
            signaling_line: default_signaling_line(),
            settings_rev: 1,
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
        if settings.settings_rev < 1 {
            if settings.signaling_line.trim() != "2" {
                settings.signaling_line = "auto".into();
            }
            settings.settings_rev = 1;
        }
        settings.normalize();
        let _ = settings.save(dir);
        Ok(settings)
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(settings_path(dir), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn is_auto(&self) -> bool {
        let line = self.signaling_line.trim();
        line.is_empty() || line == "auto"
    }

    pub fn normalize(&mut self) {
        if let Ok(url) = std::env::var("REMOTEX_SIGNALING") {
            if !url.trim().is_empty() {
                self.signaling_url = url;
                return;
            }
        }
        let line = self.signaling_line.trim();
        if line == "2" {
            self.signaling_line = "2".into();
            self.signaling_url = LINE2_SIGNALING_URL.to_string();
        } else if line == "1" {
            self.signaling_line = "1".into();
            self.signaling_url = PUBLIC_SIGNALING_URL.to_string();
        } else {
            self.signaling_line = "auto".into();
            if self.signaling_url != PUBLIC_SIGNALING_URL
                && self.signaling_url != LINE2_SIGNALING_URL
            {
                self.signaling_url = PUBLIC_SIGNALING_URL.to_string();
            }
        }
        if self.settings_rev < 1 {
            self.settings_rev = 1;
        }
    }
}

pub fn url_for_line(line: &str) -> &'static str {
    if line.trim() == "2" {
        LINE2_SIGNALING_URL
    } else {
        PUBLIC_SIGNALING_URL
    }
}

pub fn other_line_url(url: &str) -> &'static str {
    if url.contains("8.222.218.229") {
        PUBLIC_SIGNALING_URL
    } else {
        LINE2_SIGNALING_URL
    }
}

pub fn line_of_url(url: &str) -> &'static str {
    if url.contains("8.222.218.229") {
        "2"
    } else {
        "1"
    }
}

pub fn default_signaling_url() -> String {
    std::env::var("REMOTEX_SIGNALING")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| PUBLIC_SIGNALING_URL.to_string())
}

pub async fn probe_best_line() -> LinePick {
    let (rtt1, rtt2) = tokio::join!(
        probe_rtt(PUBLIC_SIGNALING_URL),
        probe_rtt(LINE2_SIGNALING_URL)
    );
    let line1_rtt_ms = rtt1.unwrap_or(0);
    let line2_rtt_ms = rtt2.unwrap_or(0);
    let (line, url) = match (rtt1, rtt2) {
        (Some(a), Some(b)) if b + 12 < a => ("2", LINE2_SIGNALING_URL),
        (None, Some(_)) => ("2", LINE2_SIGNALING_URL),
        (Some(_), None) | (Some(_), Some(_)) => ("1", PUBLIC_SIGNALING_URL),
        _ => ("1", PUBLIC_SIGNALING_URL),
    };
    LinePick {
        line: line.into(),
        url: url.into(),
        line1_rtt_ms,
        line2_rtt_ms,
    }
}

async fn probe_rtt(ws_url: &str) -> Option<u32> {
    let http = ws_url
        .replacen("ws://", "http://", 1)
        .replacen("wss://", "https://", 1);
    let http = format!(
        "{}/health",
        http.trim_end_matches("/ws").trim_end_matches('/')
    );
    let parsed = url::Url::parse(&http).ok()?;
    let host = parsed.host_str()?.to_string();
    let port = parsed.port_or_known_default()?;
    let path = parsed.path().to_string();
    let start = Instant::now();
    let ok = tokio::time::timeout(Duration::from_millis(1800), async {
        let mut stream = tokio::net::TcpStream::connect((host.as_str(), port)).await?;
        let req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
        stream.write_all(req.as_bytes()).await?;
        let mut buf = vec![0u8; 1024];
        let n = stream.read(&mut buf).await?;
        let body = String::from_utf8_lossy(&buf[..n]);
        crate::Result::Ok(body.contains("\"ok\":true"))
    })
    .await
    .ok()?
    .ok()?;
    if ok {
        Some(start.elapsed().as_millis().max(1) as u32)
    } else {
        None
    }
}

fn default_signaling_line() -> String {
    "auto".into()
}

fn settings_path(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}
