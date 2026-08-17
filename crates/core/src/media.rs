use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use capture::{capture_primary_jpeg, quality_max_width, CaptureError};
use input::{inject, InputEvent};
use protocol::ClientMsg;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRole {
    Host,
    Viewer,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteFrame {
    pub width: u32,
    pub height: u32,
    pub data: String,
}

pub struct MediaHandle {
    stop: watch::Sender<bool>,
}

impl MediaHandle {
    pub fn stop(self) {
        let _ = self.stop.send(true);
    }
}

pub fn start_host(
    session_id: String,
    quality: String,
    outgoing: mpsc::Sender<ClientMsg>,
    peer_outgoing: Option<mpsc::Sender<ClientMsg>>,
    preview_tx: Option<mpsc::Sender<RemoteFrame>>,
) -> MediaHandle {
    let (stop_tx, mut stop_rx) = watch::channel(false);
    tokio::spawn(async move {
        let mut seq = 0u64;
        let mut sent_bytes = 0u64;
        let mut window_start = Instant::now();
        let mut interval = tokio::time::interval(Duration::from_millis(90));
        loop {
            if *stop_rx.borrow() {
                break;
            }
            tokio::select! {
                changed = stop_rx.changed() => {
                    if changed.is_ok() && *stop_rx.borrow() {
                        break;
                    }
                }
                _ = interval.tick() => {
                    let max_width = quality_max_width(&quality);
                    let frame = tokio::task::spawn_blocking(move || capture_primary_jpeg(max_width, 72)).await;
                    let Ok(Ok(frame)) = frame else {
                        tracing::warn!("screen capture failed (check Screen Recording permission on macOS)");
                        continue;
                    };
                    sent_bytes += frame.bytes.len() as u64;
                    seq += 1;
                    let encoded = B64.encode(&frame.bytes);
                    if let Some(tx) = &preview_tx {
                        let _ = tx.try_send(RemoteFrame {
                            width: frame.width,
                            height: frame.height,
                            data: encoded.clone(),
                        });
                    }
                    let payload = json!({
                        "kind": "frame",
                        "seq": seq,
                        "width": frame.width,
                        "height": frame.height,
                        "data": encoded,
                    });
                    send_signal(&session_id, payload, &outgoing, peer_outgoing.as_ref()).await;
                    if window_start.elapsed() >= Duration::from_secs(1) {
                        let _ = sent_bytes;
                        window_start = Instant::now();
                        sent_bytes = 0;
                    }
                }
            }
        }
    });
    MediaHandle { stop: stop_tx }
}

pub fn handle_signal(
    data: &Value,
    role: SessionRole,
    frame_tx: &mpsc::Sender<RemoteFrame>,
    host_screen: &Arc<Mutex<(u32, u32)>>,
) {
    match data.get("kind").and_then(Value::as_str) {
        Some("frame") if matches!(role, SessionRole::Viewer) => {
            let Some(width) = data.get("width").and_then(Value::as_u64) else {
                return;
            };
            let Some(height) = data.get("height").and_then(Value::as_u64) else {
                return;
            };
            let Some(data) = data.get("data").and_then(Value::as_str) else {
                return;
            };
            let _ = frame_tx.try_send(RemoteFrame {
                width: width as u32,
                height: height as u32,
                data: data.to_string(),
            });
        }
        Some("input") if matches!(role, SessionRole::Host) => {
            let Some(event_value) = data.get("event") else {
                return;
            };
            if let Ok(event) = serde_json::from_value::<InputEvent>(event_value.clone()) {
                let screen = host_screen
                    .try_lock()
                    .map(|s| *s)
                    .unwrap_or((1920, 1080));
                inject(&event, screen);
            }
        }
        _ => {}
    }
}

pub async fn send_input_signal(
    session_id: &str,
    event: InputEvent,
    outgoing: &mpsc::Sender<ClientMsg>,
    peer_outgoing: Option<&mpsc::Sender<ClientMsg>>,
) {
    let payload = json!({
        "kind": "input",
        "event": event,
    });
    send_signal(session_id, payload, outgoing, peer_outgoing).await;
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

pub fn primary_screen_size() -> (u32, u32) {
    match capture::list_displays() {
        Ok(list) => {
            if let Some(primary) = list.iter().find(|d| d.is_primary) {
                return (primary.width, primary.height);
            }
            if let Some(first) = list.first() {
                return (first.width, first.height);
            }
            (1920, 1080)
        }
        Err(_) => (1920, 1080),
    }
}

#[allow(dead_code)]
pub fn capture_error_message(err: CaptureError) -> String {
    err.to_string()
}
