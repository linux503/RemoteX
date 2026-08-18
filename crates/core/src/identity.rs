use protocol::{format_device_id, OsKind};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub name: String,
    pub os: OsKind,
}

impl DeviceIdentity {
    pub fn load_or_create(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = identity_path(dir);
        if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            let stored: StoredIdentity = serde_json::from_str(&raw)?;
            Ok(Self {
                device_id: stored.device_id,
                name: local_device_name(),
                os: OsKind::current(),
            })
        } else {
            let identity = Self {
                device_id: generate_device_id(),
                name: local_device_name(),
                os: OsKind::current(),
            };
            identity.save(dir)?;
            Ok(identity)
        }
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let stored = StoredIdentity {
            device_id: self.device_id.clone(),
        };
        std::fs::write(identity_path(dir), serde_json::to_string_pretty(&stored)?)?;
        Ok(())
    }

    pub fn formatted_id(&self) -> String {
        format_device_id(&self.device_id)
    }
}

#[derive(Serialize, Deserialize)]
struct StoredIdentity {
    device_id: String,
}

pub fn identity_path(dir: &Path) -> PathBuf {
    dir.join("device.json")
}

pub fn generate_device_id() -> String {
    let n: u32 = rand::thread_rng().gen_range(100_000_000..=999_999_999);
    format!("{n:09}")
}

pub fn generate_password() -> String {
    const CHARS: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut rng = rand::thread_rng();
    (0..6)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

pub fn format_password(password: &str) -> String {
    password
        .chars()
        .map(|c| c.to_ascii_uppercase().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn local_device_name() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| whoami::devicename())
}

pub fn data_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("REMOTEX_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    dirs::data_dir()
        .map(|p| p.join("RemoteX"))
        .ok_or_else(|| Error::Message("cannot resolve data directory".into()))
}
