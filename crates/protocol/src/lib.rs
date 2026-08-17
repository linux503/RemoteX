use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_SIGNALING_PORT: u16 = 7829;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OsKind {
    Macos,
    Windows,
    Linux,
    Unknown,
}

impl OsKind {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Unknown
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Smooth,
    Balanced,
    High,
    Original,
}

impl Default for Quality {
    fn default() -> Self {
        Self::Balanced
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionPath {
    DirectP2p,
    Relay,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub os: OsKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Register {
        device_id: String,
        name: String,
        os: OsKind,
        protocol: u32,
    },
    Heartbeat,
    Connect {
        target_id: String,
        password: String,
        from_name: String,
        from_os: OsKind,
    },
    Accept {
        session_id: String,
        unattended: bool,
    },
    Decline {
        session_id: String,
    },
    AuthFailed {
        session_id: String,
    },
    Hangup {
        session_id: String,
    },
    Signal {
        session_id: String,
        data: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Registered {
        device_id: String,
    },
    Incoming {
        session_id: String,
        from: DeviceInfo,
        password: String,
    },
    Accepted {
        session_id: String,
        peer: DeviceInfo,
    },
    Declined {
        session_id: String,
    },
    AuthFailed {
        session_id: Option<String>,
        message: String,
    },
    PeerOffline {
        target_id: String,
    },
    Hangup {
        session_id: String,
    },
    Signal {
        session_id: String,
        data: serde_json::Value,
    },
    HeartbeatAck,
    Error {
        message: String,
    },
}

impl ClientMsg {
    pub fn register(device: &DeviceInfo) -> Self {
        Self::Register {
            device_id: device.id.clone(),
            name: device.name.clone(),
            os: device.os.clone(),
            protocol: PROTOCOL_VERSION,
        }
    }
}

pub fn new_session_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn normalize_device_id(raw: &str) -> String {
    raw.chars().filter(|c| c.is_ascii_digit()).collect()
}

pub fn format_device_id(id: &str) -> String {
    let digits = normalize_device_id(id);
    if digits.len() == 9 {
        format!("{} {} {}", &digits[0..3], &digits[3..6], &digits[6..9])
    } else {
        digits
    }
}

pub fn is_valid_device_id(id: &str) -> bool {
    let digits = normalize_device_id(id);
    digits.len() == 9 && !digits.starts_with('0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_nine_digit_id() {
        assert_eq!(format_device_id("825391726"), "825 391 726");
        assert!(is_valid_device_id("825 391 726"));
        assert!(!is_valid_device_id("012345678"));
    }
}
