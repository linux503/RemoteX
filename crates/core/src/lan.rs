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

        let send_sock = socket.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;
                let beacon = Packet {
                    v: 1,
                    id: Some(self_id.clone()),
                    name: Some(name.clone()),
                    os: Some(os_label(&os)),
                    ws: Some(local_ws()),
                    q: None,
                };
                if let Ok(bytes) = serde_json::to_vec(&beacon) {
                    let addr = SocketAddr::from(([255, 255, 255, 255], DEFAULT_DISCOVERY_PORT));
                    let _ = send_sock.send_to(&bytes, addr).await;
                }
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
    let Ok(ifaces) = if_addrs::get_if_addrs() else {
        return fallback_route_ip();
    };
    let mut scored: Vec<(i32, String)> = Vec::new();
    for iface in ifaces {
        if iface.is_loopback() || skip_iface(&iface.name) {
            continue;
        }
        let std::net::IpAddr::V4(ip) = iface.ip() else {
            continue;
        };
        if !is_wifi_lan_ipv4(ip) {
            continue;
        }
        let octets = ip.octets();
        let mut score = 10;
        let name = iface.name.to_lowercase();
        if name.starts_with("en") || name.contains("wi-fi") || name.contains("wifi") || name.contains("wlan") {
            score += 50;
        } else if name.starts_with("eth") || name.starts_with("ethernet") {
            score += 30;
        }
        if octets[0] == 192 && octets[1] == 168 {
            score += 20;
        } else if octets[0] == 10 {
            score += 10;
        }
        scored.push((score, ip.to_string()));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().next().map(|(_, ip)| ip).or_else(fallback_route_ip)
}

fn fallback_route_ip() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) if is_wifi_lan_ipv4(ip) => Some(ip.to_string()),
        _ => None,
    }
}

fn skip_iface(name: &str) -> bool {
    let name = name.to_lowercase();
    name.starts_with("utun")
        || name.starts_with("awdl")
        || name.starts_with("llw")
        || name.starts_with("bridge")
        || name.starts_with("ipsec")
        || name.starts_with("tun")
        || name.starts_with("tap")
        || name.starts_with("wg")
        || name.starts_with("zt")
        || name.contains("vpn")
        || name.contains("tailscale")
        || name.contains("vethernet")
}

fn is_wifi_lan_ipv4(ip: std::net::Ipv4Addr) -> bool {
    if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() || ip.is_multicast() {
        return false;
    }
    let o = ip.octets();
    // Clash / Surge / RFC 2544 fake subnet — not Wi-Fi
    if o[0] == 198 && (o[1] == 18 || o[1] == 19) {
        return false;
    }
    // CGNAT (100.64/10), often VPN
    if o[0] == 100 && (64..128).contains(&o[1]) {
        return false;
    }
    ip.is_private()
}

pub fn is_fake_vpn_signaling(url: &str) -> bool {
    url.contains("198.18.") || url.contains("198.19.") || url.contains("169.254.")
}

pub fn is_own_hub(url: &str) -> bool {
    url.contains("127.0.0.1")
        || url.contains("localhost")
        || local_lan_ip().is_some_and(|ip| url.contains(&ip))
}

pub fn is_lan_url(url: &str) -> bool {
    if is_own_hub(url) {
        return true;
    }
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let Ok(ip) = host.parse::<std::net::Ipv4Addr>() else {
        return false;
    };
    ip.is_private() || ip.is_loopback()
}

pub async fn hub_reachable(ws: &str) -> bool {
    let Ok(parsed) = url::Url::parse(ws) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let port = parsed.port_or_known_default().unwrap_or(DEFAULT_SIGNALING_PORT);
    tokio::time::timeout(
        Duration::from_millis(350),
        tokio::net::TcpStream::connect((host, port)),
    )
    .await
    .ok()
    .and_then(Result::ok)
    .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn rejects_clash_tun() {
        assert!(!is_wifi_lan_ipv4(Ipv4Addr::new(198, 18, 0, 1)));
        assert!(is_wifi_lan_ipv4(Ipv4Addr::new(192, 168, 1, 18)));
        assert!(is_fake_vpn_signaling("ws://198.18.0.1:7829/ws"));
    }

    #[test]
    fn lan_url_detects_private_ip() {
        assert!(is_lan_url("ws://192.168.1.18:7829/ws"));
        assert!(is_lan_url("ws://10.0.0.8:7829/ws"));
        assert!(is_lan_url("ws://127.0.0.1:7829/ws"));
        assert!(!is_lan_url("ws://23.226.134.88:7829/ws"));
    }

    #[test]
    fn local_ip_is_real_lan() {
        if let Some(ip) = local_lan_ip() {
            assert!(!ip.starts_with("198.18"), "got {ip}");
            assert!(!ip.starts_with("127."));
        }
    }
}
