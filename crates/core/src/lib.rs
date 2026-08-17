pub mod identity;
pub mod password;
pub mod recents;
pub mod settings;
pub mod signaling;
pub mod state;

pub use identity::{DeviceIdentity, format_password};
pub use password::PasswordVault;
pub use recents::{RecentDevice, RecentsStore};
pub use settings::AppSettings;
pub use signaling::SignalingClient;
pub use state::{AppEvent, AppState, SessionPhase, Snapshot};

use std::sync::atomic::AtomicBool;
use thiserror::Error;

pub static HOSTING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Ws(#[from] tokio_tungstenite::tungstenite::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
