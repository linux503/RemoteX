use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use protocol::{new_session_id, normalize_device_id, ClientMsg, DeviceInfo, ServerMsg};
use serde::Serialize;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

pub static HOSTING: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
struct Hub {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    devices: HashMap<String, DeviceConn>,
    sessions: HashMap<String, Session>,
}

struct DeviceConn {
    tx: mpsc::UnboundedSender<ServerMsg>,
    info: DeviceInfo,
    last_seen: std::time::Instant,
}

#[derive(Clone)]
struct Session {
    caller_id: String,
    callee_id: String,
}

#[derive(Serialize)]
struct Health {
    ok: bool,
    devices: usize,
    sessions: usize,
}

pub fn lan_signaling_url(port: u16) -> String {
    format!("ws://{}:{port}/ws", lan_ip().unwrap_or_else(|| "127.0.0.1".into()))
}

pub fn lan_ip() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()),
        _ => None,
    }
}

pub async fn serve(addr: SocketAddr) -> std::io::Result<()> {
    let hub = Hub {
        inner: Arc::new(Mutex::new(Inner {
            devices: HashMap::new(),
            sessions: HashMap::new(),
        })),
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_handler))
        .with_state(hub.clone());
    let listener = tokio::net::TcpListener::bind(addr).await?;
    HOSTING.store(true, Ordering::SeqCst);
    info!("RemoteX signaling on ws://{addr}/ws");
    tokio::spawn(async move {
        sweep_stale(hub).await;
    });
    axum::serve(listener, app).await
}

async fn health(State(hub): State<Hub>) -> Json<Health> {
    let inner = hub.inner.lock().await;
    Json(Health {
        ok: true,
        devices: inner.devices.len(),
        sessions: inner.sessions.len(),
    })
}

async fn ws_handler(ws: WebSocketUpgrade, State(hub): State<Hub>) -> impl IntoResponse {
    ws.max_message_size(16 * 1024 * 1024)
        .max_frame_size(16 * 1024 * 1024)
        .on_upgrade(move |socket| handle_socket(socket, hub))
}

async fn handle_socket(socket: WebSocket, hub: Hub) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();
    let mut device_id: Option<String> = None;

    loop {
        tokio::select! {
            outgoing = rx.recv() => {
                let Some(msg) = outgoing else { break; };
                let Ok(json) = serde_json::to_string(&msg) else { continue; };
                if sink.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            incoming = stream.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMsg>(&text) {
                            Ok(msg) => {
                                if let Err(err) = handle_client(&hub, &tx, &mut device_id, msg).await {
                                    let _ = tx.send(ServerMsg::Error { message: err });
                                }
                            }
                            Err(err) => {
                                let _ = tx.send(ServerMsg::Error { message: err.to_string() });
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = sink.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(err)) => {
                        warn!("websocket error: {err}");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(id) = device_id {
        let mut inner = hub.inner.lock().await;
        if inner
            .devices
            .get(&id)
            .map(|d| d.tx.same_channel(&tx))
            .unwrap_or(false)
        {
            drop_device(&mut inner, &id);
        }
    }
}

async fn sweep_stale(hub: Hub) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
    loop {
        interval.tick().await;
        let mut inner = hub.inner.lock().await;
        let stale: Vec<String> = inner
            .devices
            .iter()
            .filter(|(_, conn)| conn.last_seen.elapsed() > std::time::Duration::from_secs(8))
            .map(|(id, _)| id.clone())
            .collect();
        for id in stale {
            info!("device {id} timed out");
            drop_device(&mut inner, &id);
        }
    }
}

fn drop_device(inner: &mut Inner, id: &str) {
    inner.devices.remove(id);
    info!("device {id} disconnected");
    let stale: Vec<_> = inner
        .sessions
        .iter()
        .filter(|(_, s)| s.caller_id == id || s.callee_id == id)
        .map(|(sid, s)| (sid.clone(), s.clone()))
        .collect();
    for (sid, session) in stale {
        inner.sessions.remove(&sid);
        let peer = if session.caller_id == id {
            session.callee_id
        } else {
            session.caller_id
        };
        if let Some(conn) = inner.devices.get(&peer) {
            let _ = conn.tx.send(ServerMsg::Hangup {
                session_id: sid,
            });
        }
    }
}

async fn handle_client(
    hub: &Hub,
    tx: &mpsc::UnboundedSender<ServerMsg>,
    device_id: &mut Option<String>,
    msg: ClientMsg,
) -> Result<(), String> {
    if let Some(id) = device_id.as_ref() {
        if let Some(conn) = hub.inner.lock().await.devices.get_mut(id) {
            conn.last_seen = std::time::Instant::now();
        }
    }
    match msg {
        ClientMsg::Register {
            device_id: id,
            name,
            os,
            ..
        } => {
            let id = normalize_device_id(&id);
            if !protocol::is_valid_device_id(&id) {
                return Err("invalid device id".into());
            }
            let info = DeviceInfo {
                id: id.clone(),
                name,
                os,
            };
            let mut inner = hub.inner.lock().await;
            inner.devices.insert(
                id.clone(),
                DeviceConn {
                    tx: tx.clone(),
                    info,
                    last_seen: std::time::Instant::now(),
                },
            );
            *device_id = Some(id.clone());
            let _ = tx.send(ServerMsg::Registered { device_id: id.clone() });
            info!("device {id} registered");
            Ok(())
        }
        ClientMsg::Heartbeat => {
            let _ = tx.send(ServerMsg::HeartbeatAck);
            Ok(())
        }
        ClientMsg::Connect {
            target_id,
            password,
            from_name,
            from_os,
        } => {
            let caller_id = device_id.clone().ok_or("register first")?;
            let target_id = normalize_device_id(&target_id);
            let mut inner = hub.inner.lock().await;
            if !inner.devices.contains_key(&target_id) {
                let _ = tx.send(ServerMsg::PeerOffline {
                    target_id: target_id.clone(),
                });
                return Ok(());
            }
            let session_id = new_session_id();
            inner.sessions.insert(
                session_id.clone(),
                Session {
                    caller_id: caller_id.clone(),
                    callee_id: target_id.clone(),
                },
            );
            let from = DeviceInfo {
                id: caller_id,
                name: from_name,
                os: from_os,
            };
            if let Some(target) = inner.devices.get(&target_id) {
                let _ = target.tx.send(ServerMsg::Incoming {
                    session_id,
                    from,
                    password,
                });
            }
            Ok(())
        }
        ClientMsg::Accept {
            session_id,
            unattended: _,
        } => {
            let inner = hub.inner.lock().await;
            let session = inner
                .sessions
                .get(&session_id)
                .ok_or("unknown session")?
                .clone();
            let caller = inner.devices.get(&session.caller_id).map(|d| d.info.clone());
            let callee = inner.devices.get(&session.callee_id).map(|d| d.info.clone());
            if let (Some(caller), Some(callee)) = (caller, callee) {
                if let Some(conn) = inner.devices.get(&session.caller_id) {
                    let _ = conn.tx.send(ServerMsg::Accepted {
                        session_id: session_id.clone(),
                        peer: callee.clone(),
                    });
                }
                if let Some(conn) = inner.devices.get(&session.callee_id) {
                    let _ = conn.tx.send(ServerMsg::Accepted {
                        session_id,
                        peer: caller,
                    });
                }
            }
            Ok(())
        }
        ClientMsg::Decline { session_id } => {
            let mut inner = hub.inner.lock().await;
            if let Some(session) = inner.sessions.remove(&session_id) {
                if let Some(conn) = inner.devices.get(&session.caller_id) {
                    let _ = conn.tx.send(ServerMsg::Declined {
                        session_id: session_id.clone(),
                    });
                }
            }
            Ok(())
        }
        ClientMsg::AuthFailed { session_id } => {
            let mut inner = hub.inner.lock().await;
            if let Some(session) = inner.sessions.remove(&session_id) {
                if let Some(conn) = inner.devices.get(&session.caller_id) {
                    let _ = conn.tx.send(ServerMsg::AuthFailed {
                        session_id: Some(session_id),
                        message: "Incorrect password".into(),
                    });
                }
            }
            Ok(())
        }
        ClientMsg::Hangup { session_id } => {
            let mut inner = hub.inner.lock().await;
            if let Some(session) = inner.sessions.remove(&session_id) {
                for id in [session.caller_id, session.callee_id] {
                    if let Some(conn) = inner.devices.get(&id) {
                        let _ = conn.tx.send(ServerMsg::Hangup {
                            session_id: session_id.clone(),
                        });
                    }
                }
            }
            Ok(())
        }
        ClientMsg::Signal { session_id, data } => {
            let inner = hub.inner.lock().await;
            let Some(session) = inner.sessions.get(&session_id) else {
                return Ok(());
            };
            let self_id = device_id.clone().ok_or("register first")?;
            let peer = if self_id == session.caller_id {
                session.callee_id.clone()
            } else {
                session.caller_id.clone()
            };
            if let Some(conn) = inner.devices.get(&peer) {
                let _ = conn.tx.send(ServerMsg::Signal { session_id, data });
            }
            Ok(())
        }
    }
}
