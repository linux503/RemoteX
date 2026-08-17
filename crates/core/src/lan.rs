use protocol::{OsKind, DEFAULT_DISCOVERY_PORT, DEFAULT_SIGNALING_PORT};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearbyDevice {
    pub id: String,
    pub name: String,
    pub os: String,
    pub ws: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Packet {
    v: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    os: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ws: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    q: Option<String>,
}

struct Seen {
    device: NearbyDevice,
    at: Instant,
}

pub struct LanDiscovery {
    socket: Arc<UdpSocket>,
    nearby: Arc<Mutex<HashMap<String, Seen>>>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<NearbyDevice>>>>,
    self_id: String,
}

impl LanDiscovery {
    pub async fn start(self_id: String, name: String, os: OsKind) -> std::io::Result<Arc<Self>> {
        let socket = match UdpSocket::bind(("0.0.0.0", DEFAULT_DISCOVERY_PORT)).await {
            Ok(s) => s,
            Err(_) => UdpSocket::bind(("0.0.0.0", 0)).await?,
        };
        socket.set_broadcast(true)?;
        let socket = Arc::new(socket);
        let lan = Arc::new(Self {
            socket: socket.clone(),
            nearby: Arc::new(Mutex::new(HashMap::new())),
            pending: Arc::new(Mutex::new(HashMap::new())),
            self_id: self_id.clone(),
        });

        let beacon = Packet {
            v: 1,
            id: Some(self_id),
            name: Some(name),
            os: Some(os_label(&os)),
            ws: Some(local_ws()),
            q: None,
        };
        let beacon_bytes = serde_json::to_vec(&beacon).unwrap_or_default();
        let send_sock = socket.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;
                let addr = SocketAddr::from(([255, 255, 255, 255], DEFAULT_DISCOVERY_PORT));
                let _ = send_sock.send_to(&beacon_bytes, addr).await;
            }
        });

        let recv = lan.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            loop {
                match recv.socket.recv_from(&mut buf).await {
                    Ok((n, from)) => recv.handle_packet(&buf[..n], from).await,
                    Err(err) => {
                        warn!("lan discovery recv: {err}");
                        tokio::time::sleep(Duration::from_millis(400)).await;
                    }
                }
            }
        });

        Ok(lan)
    }

    pub async fn list(&self) -> Vec<NearbyDevice> {
        let mut map = self.nearby.lock().await;
        map.retain(|_, seen| seen.at.elapsed() < Duration::from_secs(8));
        map.values().map(|s| s.device.clone()).collect()
    }

    pub async fn lookup(&self, id: &str) -> Option<NearbyDevice> {
        {
            let map = self.nearby.lock().await;
            if let Some(seen) = map.get(id) {
                if seen.at.elapsed() < Duration::from_secs(8) {
                    return Some(seen.device.clone());
                }
            }
        }
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id.to_string(), tx);
        let query = Packet {
            v: 1,
            id: None,
            name: None,
            os: None,
            ws: None,
            q: Some(id.to_string()),
        };
        if let Ok(bytes) = serde_json::to_vec(&query) {
            let addr = SocketAddr::from(([255, 255, 255, 255], DEFAULT_DISCOVERY_PORT));
            let _ = self.socket.send_to(&bytes, addr).await;
        }
        match tokio::time::timeout(Duration::from_millis(800), rx).await {
            Ok(Ok(device)) => Some(device),
            _ => {
                self.pending.lock().await.remove(id);
                None
            }
        }
    }

    async fn handle_packet(&self, bytes: &[u8], from: SocketAddr) {
        let Ok(pkt) = serde_json::from_slice::<Packet>(bytes) else {
            return;
        };
        if pkt.v != 1 {
            return;
        }
        if let Some(q) = pkt.q.as_deref() {
            if q == self.self_id {
                let reply = Packet {
                    v: 1,
                    id: Some(self.self_id.clone()),
                    name: None,
                    os: None,
                    ws: Some(local_ws()),
                    q: None,
                };
                if let Ok(bytes) = serde_json::to_vec(&reply) {
                    let _ = self.socket.send_to(&bytes, from).await;
                }
            }
            return;
        }
        let Some(id) = pkt.id.clone() else { return };
        if id == self.self_id {
            return;
        }
        let Some(ws) = pkt.ws.clone() else { return };
        let device = NearbyDevice {
            id: id.clone(),
            name: pkt.name.clone().unwrap_or_else(|| id.clone()),
            os: pkt.os.clone().unwrap_or_else(|| "unknown".into()),
            ws,
        };
        debug!("lan nearby {} {}", device.name, device.ws);
        self.nearby.lock().await.insert(
            id.clone(),
            Seen {
                device: device.clone(),
                at: Instant::now(),
            },
        );
        if let Some(tx) = self.pending.lock().await.remove(&id) {
            let _ = tx.send(device);
        }
    }
}

fn os_label(os: &OsKind) -> String {
    match os {
        OsKind::Macos => "macos".into(),
        OsKind::Windows => "windows".into(),
        OsKind::Linux => "linux".into(),
        OsKind::Unknown => "unknown".into(),
    }
}

pub fn local_ws() -> String {
    format!(
        "ws://{}:{}/ws",
        local_lan_ip().unwrap_or_else(|| "127.0.0.1".into()),
        DEFAULT_SIGNALING_PORT
    )
}

pub fn local_lan_ip() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()),
        _ => None,
    }
}

pub fn is_own_hub(url: &str) -> bool {
    url.contains("127.0.0.1")
        || url.contains("localhost")
        || local_lan_ip().is_some_and(|ip| url.contains(&ip))
}
