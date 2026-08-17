use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use capture::{capture_primary_jpeg, quality_params, CaptureError};
use input::{inject, InputEvent};
use protocol::ClientMsg;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
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
    quality_rx: watch::Receiver<String>,
    outgoing: mpsc::Sender<ClientMsg>,
    peer_outgoing: Option<mpsc::Sender<ClientMsg>>,
    preview_tx: Option<watch::Sender<Option<RemoteFrame>>>,
) -> MediaHandle {
    let (stop_tx, stop_rx) = watch::channel(false);
    let (latest_out, latest_rx) = watch::channel(None::<Value>);

    let capture_stop = stop_rx.clone();
    let mut quality_rx = quality_rx;
    tokio::spawn(async move {
        let mut failures = 0u32;
        loop {
            if *capture_stop.borrow() {
                break;
            }
            let quality = quality_rx.borrow().clone();
            let (max_width, jpeg_q, wait_ms) = quality_params(&quality);
            tokio::select! {
                changed = stop_watch(capture_stop.clone()) => {
                    if changed {
                        break;
                    }
                }
                changed = quality_rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(wait_ms)) => {
                    let frame = tokio::task::spawn_blocking(move || capture_primary_jpeg(max_width, jpeg_q)).await;
                    let Ok(Ok(frame)) = frame else {
                        failures = failures.saturating_add(1);
                        if failures == 1 || failures % 20 == 0 {
                            tracing::warn!("screen capture failed (check Screen Recording permission on macOS)");
                        }
                        continue;
                    };
                    failures = 0;
                    let encoded = B64.encode(&frame.bytes);
                    let remote = RemoteFrame {
                        width: frame.width,
                        height: frame.height,
                        data: encoded.clone(),
                    };
                    if let Some(tx) = &preview_tx {
                        let _ = tx.send(Some(remote));
                    }
                    let payload = json!({
                        "kind": "frame",
                        "width": frame.width,
                        "height": frame.height,
                        "data": encoded,
                    });
                    let _ = latest_out.send(Some(payload));
                }
            }
        }
    });

    let mut send_stop = stop_rx;
    tokio::spawn(async move {
        let mut latest_rx = latest_rx;
        loop {
            tokio::select! {
                changed = send_stop.changed() => {
                    if changed.is_ok() && *send_stop.borrow() {
                        break;
                    }
                }
                changed = latest_rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    let payload = latest_rx.borrow_and_update().clone();
                    let Some(payload) = payload else { continue; };
                    send_signal(&session_id, payload, &outgoing, peer_outgoing.as_ref()).await;
                }
            }
        }
    });

    MediaHandle { stop: stop_tx }
}

async fn stop_watch(mut rx: watch::Receiver<bool>) -> bool {
    rx.changed().await.is_ok() && *rx.borrow()
}

pub fn handle_signal(
    data: &Value,
    role: SessionRole,
    frame_tx: &watch::Sender<Option<RemoteFrame>>,
    host_screen: &Arc<Mutex<(u32, u32)>>,
    quality_tx: &watch::Sender<String>,
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
            let _ = frame_tx.send(Some(RemoteFrame {
                width: width as u32,
                height: height as u32,
                data: data.to_string(),
            }));
        }
        Some("quality") if matches!(role, SessionRole::Host) => {
            if let Some(value) = data.get("value").and_then(Value::as_str) {
                if matches!(value, "smooth" | "balanced" | "high" | "original") {
                    let _ = quality_tx.send(value.to_string());
                }
            }
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

pub async fn send_quality_signal(
    session_id: &str,
    quality: &str,
    outgoing: &mpsc::Sender<ClientMsg>,
    peer_outgoing: Option<&mpsc::Sender<ClientMsg>>,
) {
    let payload = json!({
        "kind": "quality",
        "value": quality,
    });
    send_signal(session_id, payload, outgoing, peer_outgoing).await;
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
