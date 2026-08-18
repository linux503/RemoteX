use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use protocol::ClientMsg;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::Result;

const CHUNK_SIZE: usize = 48 * 1024;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct TransferProgress {
    pub name: String,
    pub percent: u8,
    pub direction: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferComplete {
    pub name: String,
    pub path: String,
}

struct IncomingFile {
    name: String,
    size: u64,
    buffer: Vec<u8>,
}

pub struct TransferHub {
    incoming: HashMap<String, IncomingFile>,
}

impl Default for TransferHub {
    fn default() -> Self {
        Self {
            incoming: HashMap::new(),
        }
    }
}

impl TransferHub {
    pub fn handle(
        &mut self,
        data: &Value,
        allow: bool,
    ) -> Result<Option<TransferComplete>> {
        if !allow {
            return Ok(None);
        }
        match data.get("kind").and_then(Value::as_str) {
            Some("file_begin") => {
                let id = data
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let name = sanitize_filename(
                    data.get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("download.bin"),
                );
                let size = data.get("size").and_then(Value::as_u64).unwrap_or(0);
                if id.is_empty() || size == 0 || size > MAX_FILE_BYTES {
                    return Ok(None);
                }
                self.incoming.insert(
                    id,
                    IncomingFile {
                        name,
                        size,
                        buffer: Vec::with_capacity(size.min(8 * 1024 * 1024) as usize),
                    },
                );
                Ok(None)
            }
            Some("file_chunk") => {
                let id = data.get("id").and_then(Value::as_str).unwrap_or("");
                let index = data.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let chunk = data.get("data").and_then(Value::as_str).unwrap_or("");
                let Some(file) = self.incoming.get_mut(id) else {
                    return Ok(None);
                };
                let bytes = B64.decode(chunk).map_err(|e| crate::Error::Message(e.to_string()))?;
                let offset = index * CHUNK_SIZE;
                if file.buffer.len() < offset {
                    file.buffer.resize(offset, 0);
                }
                if offset == file.buffer.len() {
                    file.buffer.extend_from_slice(&bytes);
                } else if offset + bytes.len() <= file.buffer.len() {
                    file.buffer[offset..offset + bytes.len()].copy_from_slice(&bytes);
                }
                Ok(None)
            }
            Some("file_end") => {
                let id = data.get("id").and_then(Value::as_str).unwrap_or("");
                let Some(file) = self.incoming.remove(id) else {
                    return Ok(None);
                };
                if file.buffer.len() as u64 != file.size {
                    return Err(crate::Error::Message("Incomplete file transfer".into()));
                }
                let path = save_download(&file.name, &file.buffer)?;
                Ok(Some(TransferComplete {
                    name: file.name,
                    path: path.display().to_string(),
                }))
            }
            _ => Ok(None),
        }
    }

    pub fn reset(&mut self) {
        self.incoming.clear();
    }
}

pub async fn send_file(
    session_id: &str,
    path: &Path,
    outgoing: &mpsc::Sender<ClientMsg>,
    peer_outgoing: Option<&mpsc::Sender<ClientMsg>>,
) -> Result<()> {
    let meta = std::fs::metadata(path)?;
    let size = meta.len();
    if size == 0 {
        return Err(crate::Error::Message("File is empty".into()));
    }
    if size > MAX_FILE_BYTES {
        return Err(crate::Error::Message("File is too large (max 256 MB)".into()));
    }
    let name = sanitize_filename(
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file.bin"),
    );
    let id = Uuid::new_v4().to_string();
    let bytes = std::fs::read(path)?;
    send_signal(
        session_id,
        json!({ "kind": "file_begin", "id": id, "name": name, "size": size }),
        outgoing,
        peer_outgoing,
    )
    .await;
    for (index, chunk) in bytes.chunks(CHUNK_SIZE).enumerate() {
        send_signal(
            session_id,
            json!({
                "kind": "file_chunk",
                "id": id,
                "index": index,
                "data": B64.encode(chunk),
            }),
            outgoing,
            peer_outgoing,
        )
        .await;
    }
    send_signal(
        session_id,
        json!({ "kind": "file_end", "id": id }),
        outgoing,
        peer_outgoing,
    )
    .await;
    Ok(())
}

fn save_download(name: &str, bytes: &[u8]) -> Result<PathBuf> {
    let dir = dirs::download_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| crate::Error::Message("No download folder".into()))?
        .join("RemoteX");
    std::fs::create_dir_all(&dir)?;
    let mut path = dir.join(name);
    if path.exists() {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let stamped = format!(
            "{}-{}{}",
            stem,
            chrono::Local::now().format("%H%M%S"),
            if ext.is_empty() {
                "".into()
            } else {
                format!(".{ext}")
            }
        );
        path = dir.join(stamped);
    }
    std::fs::write(&path, bytes)?;
    Ok(path)
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "download.bin".into()
    } else {
        trimmed.chars().take(120).collect()
    }
}

async fn send_signal(
    session_id: &str,
    data: Value,
    outgoing: &mpsc::Sender<ClientMsg>,
    peer_outgoing: Option<&mpsc::Sender<ClientMsg>>,
) {
    let msg = ClientMsg::Signal {
        session_id: session_id.to_string(),
        data,
    };
    if let Some(peer) = peer_outgoing {
        let _ = peer.send(msg).await;
    } else {
        let _ = outgoing.send(msg).await;
    }
}
