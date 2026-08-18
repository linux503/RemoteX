use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use capture::{capture_interval_ms, capture_max_width, capture_primary_jpeg_changed, quality_params, CaptureError};
use crate::clipboard;
use crate::transfer::{TransferComplete, TransferHub};
use input::{inject, InputEvent};
use protocol::ClientMsg;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, Mutex};

#[derive(Debug, Clone)]
pub struct CapturePrefs {
    pub resolution: String,
    pub fps: u32,
    pub viewport_w: u32,
}

impl Default for CapturePrefs {
    fn default() -> Self {
        Self {
            resolution: "auto".into(),
            fps: 60,
            viewport_w: 1920,
        }
    }
}

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
    capture_prefs: Arc<Mutex<CapturePrefs>>,
    outgoing: mpsc::Sender<ClientMsg>,
    peer_outgoing: Option<mpsc::Sender<ClientMsg>>,
    lan_outgoing: Option<mpsc::Sender<ClientMsg>>,
    preview_tx: Option<watch::Sender<Option<RemoteFrame>>>,
    host_screen: Arc<Mutex<(u32, u32)>>,
    link_bytes: Arc<(AtomicU64, AtomicU64)>,
) -> MediaHandle {
    let (stop_tx, stop_rx) = watch::channel(false);
    let (latest_out, latest_rx) = watch::channel(None::<Value>);

    let capture_stop = stop_rx.clone();
    let link_stats = link_bytes.clone();
    let mut quality_rx = quality_rx;
    tokio::spawn(async move {
        let mut failures = 0u32;
        let mut last_fp = 0u64;
        let mut ticks = 0u32;
        loop {
            if *capture_stop.borrow() {
                break;
            }
            let quality = quality_rx.borrow().clone();
            let (_q_max, jpeg_q, wait_q) = quality_params(&quality);
            let prefs = capture_prefs.lock().await.clone();
            let max_width = capture_max_width(&prefs.resolution, prefs.viewport_w);
            let wait_ms = capture_interval_ms(prefs.fps, wait_q);
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
                    last_fp = 0;
                }
                    _ = tokio::time::sleep(Duration::from_millis(wait_ms)) => {
                    let frame = tokio::task::spawn_blocking(move || {
                        let mut fp = last_fp;
                        let captured = capture_primary_jpeg_changed(max_width, jpeg_q, Some(&mut fp));
                        (captured, fp)
                    }).await;
                    let Ok((captured, fp)) = frame else {
                        continue;
                    };
                    last_fp = fp;
                    let Ok(frame) = captured else {
                        failures = failures.saturating_add(1);
                        if failures == 1 || failures % 20 == 0 {
                            tracing::warn!("screen capture failed (check Screen Recording permission on macOS)");
                        }
                        continue;
                    };
                    let Some(frame) = frame else {
                        continue;
                    };
                    failures = 0;
                    ticks = ticks.saturating_add(1);
                    if ticks % 45 == 0 {
                        *host_screen.lock().await = primary_screen_size();
                    }
                    let encoded = B64.encode(&frame.bytes);
                    link_stats
                        .1
                        .fetch_add(encoded.len() as u64, Ordering::Relaxed);
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
        let mut alive = tokio::time::interval(Duration::from_millis(1500));
        alive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last_send = std::time::Instant::now();
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
                    last_send = std::time::Instant::now();
                    send_signal_latest(
                        &session_id,
                        payload,
                        &outgoing,
                        peer_outgoing.as_ref(),
                        lan_outgoing.as_ref(),
                    )
                    .await;
                }
                _ = alive.tick() => {
                    if last_send.elapsed() < Duration::from_millis(1400) {
                        continue;
                    }
                    last_send = std::time::Instant::now();
                    send_signal(
                        &session_id,
                        json!({ "kind": "keepalive" }),
                        &outgoing,
                        peer_outgoing.as_ref(),
                        lan_outgoing.as_ref(),
                    )
                    .await;
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
    allow_clipboard: bool,
    allow_file_transfer: bool,
    transfer_hub: &mut TransferHub,
    link_rx: Option<&AtomicU64>,
    _link_tx: Option<&AtomicU64>,
) -> crate::Result<Option<SignalSideEffect>> {
    let track_rx = |bytes: usize| {
        if let Some(counter) = link_rx {
            counter.fetch_add(bytes as u64, Ordering::Relaxed);
        }
    };
    match data.get("kind").and_then(Value::as_str) {
        Some("frame") if matches!(role, SessionRole::Viewer) => {
            let Some(width) = data.get("width").and_then(Value::as_u64) else {
                return Ok(None);
            };
            let Some(height) = data.get("height").and_then(Value::as_u64) else {
                return Ok(None);
            };
            let Some(data) = data.get("data").and_then(Value::as_str) else {
                return Ok(None);
            };
            track_rx(data.len());
            let _ = frame_tx.send(Some(RemoteFrame {
                width: width as u32,
                height: height as u32,
                data: data.to_string(),
            }));
        }
        Some("viewport") if matches!(role, SessionRole::Host) => {
            let width = data.get("width").and_then(Value::as_u64).unwrap_or(0) as u32;
            if width >= 640 {
                return Ok(Some(SignalSideEffect::Viewport(width)));
            }
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
                return Ok(None);
            };
            if let Ok(event) = serde_json::from_value::<InputEvent>(event_value.clone()) {
                let screen = host_screen
                    .try_lock()
                    .map(|s| *s)
                    .unwrap_or((1920, 1080));
                inject(&event, screen);
            }
        }
        Some("clipboard") if allow_clipboard => {
            let Some(text) = data.get("text").and_then(Value::as_str) else {
                return Ok(None);
            };
            if text.len() > 512_000 {
                return Ok(None);
            }
            if clipboard::write_text(text).is_ok() {
                return Ok(Some(SignalSideEffect::ClipboardSynced(text.to_string())));
            }
        }
        Some("file_begin") | Some("file_chunk") | Some("file_end") if allow_file_transfer => {
            if let Some(done) = transfer_hub.handle(data, true)? {
                return Ok(Some(SignalSideEffect::FileReceived(done)));
            }
        }
        _ => {}
    }
    Ok(None)
}

#[derive(Debug, Clone)]
pub enum SignalSideEffect {
    ClipboardSynced(String),
    FileReceived(TransferComplete),
    Viewport(u32),
}

pub async fn send_clipboard_signal(
    session_id: &str,
    text: &str,
    outgoing: &mpsc::Sender<ClientMsg>,
    peer_outgoing: Option<&mpsc::Sender<ClientMsg>>,
    lan_outgoing: Option<&mpsc::Sender<ClientMsg>>,
) {
    let payload = json!({
        "kind": "clipboard",
        "text": text,
    });
    send_signal(session_id, payload, outgoing, peer_outgoing, lan_outgoing).await;
}

pub async fn send_quality_signal(
    session_id: &str,
    quality: &str,
    outgoing: &mpsc::Sender<ClientMsg>,
    peer_outgoing: Option<&mpsc::Sender<ClientMsg>>,
    lan_outgoing: Option<&mpsc::Sender<ClientMsg>>,
) {
    let payload = json!({
        "kind": "quality",
        "value": quality,
    });
    send_signal(session_id, payload, outgoing, peer_outgoing, lan_outgoing).await;
}

pub async fn send_viewport_signal(
    session_id: &str,
    width: u32,
    height: u32,
    outgoing: &mpsc::Sender<ClientMsg>,
    peer_outgoing: Option<&mpsc::Sender<ClientMsg>>,
    lan_outgoing: Option<&mpsc::Sender<ClientMsg>>,
) {
    let payload = json!({
        "kind": "viewport",
        "width": width,
        "height": height,
    });
    send_signal(session_id, payload, outgoing, peer_outgoing, lan_outgoing).await;
}

pub async fn send_input_signal(
    session_id: &str,
    event: InputEvent,
    priority: &mpsc::Sender<ClientMsg>,
    peer_priority: Option<&mpsc::Sender<ClientMsg>>,
    lan_priority: Option<&mpsc::Sender<ClientMsg>>,
    lossy: bool,
) {
    let payload = json!({
        "kind": "input",
        "event": event,
    });
    let msg = ClientMsg::Signal {
        session_id: session_id.to_string(),
        data: payload,
    };
    dispatch_signal(msg, priority, peer_priority, lan_priority, lossy).await;
}

async fn send_signal(
    session_id: &str,
    data: Value,
    outgoing: &mpsc::Sender<ClientMsg>,
    peer_outgoing: Option<&mpsc::Sender<ClientMsg>>,
    lan_outgoing: Option<&mpsc::Sender<ClientMsg>>,
) {
    let msg = ClientMsg::Signal {
        session_id: session_id.to_string(),
        data,
    };
    dispatch_signal(msg, outgoing, peer_outgoing, lan_outgoing, false).await;
}

async fn send_signal_latest(
    session_id: &str,
    data: Value,
    outgoing: &mpsc::Sender<ClientMsg>,
    peer_outgoing: Option<&mpsc::Sender<ClientMsg>>,
    lan_outgoing: Option<&mpsc::Sender<ClientMsg>>,
) {
    let msg = ClientMsg::Signal {
        session_id: session_id.to_string(),
        data,
    };
    dispatch_signal(msg, outgoing, peer_outgoing, lan_outgoing, true).await;
}

async fn dispatch_signal(
    msg: ClientMsg,
    outgoing: &mpsc::Sender<ClientMsg>,
    peer_outgoing: Option<&mpsc::Sender<ClientMsg>>,
    lan_outgoing: Option<&mpsc::Sender<ClientMsg>>,
    lossy: bool,
) {
    let mut targets = Vec::with_capacity(2);
    if let Some(peer) = peer_outgoing {
        targets.push(peer);
    } else {
        targets.push(outgoing);
        if let Some(lan) = lan_outgoing {
            targets.push(lan);
        }
    }
    for (i, tx) in targets.into_iter().enumerate() {
        let packet = if i == 0 { msg.clone() } else { msg.clone() };
        if lossy {
            let _ = tx.try_send(packet);
        } else {
            let _ = tx.send(packet).await;
        }
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
