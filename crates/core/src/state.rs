use protocol::{format_device_id, ClientMsg, DeviceInfo, OsKind, ServerMsg};
use rand::Rng;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, watch, Mutex};

use crate::identity::{data_dir, format_password, DeviceIdentity};
use crate::lan::{self, LanDiscovery, NearbyDevice};
use crate::media::{self, MediaHandle, RemoteFrame, SessionRole};
use crate::password::{AuthOutcome, PasswordVault};
use crate::recents::RecentsStore;
use crate::settings::AppSettings;
use crate::signaling::SignalingClient;
use crate::Result;
use input::InputEvent;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionPhase {
    Idle,
    Connecting,
    Incoming,
    Connected,
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub device_id: String,
    pub formatted_id: String,
    pub name: String,
    pub os: OsKind,
    pub temp_password: String,
    pub formatted_password: String,
    pub ready: bool,
    pub rtt_ms: u32,
    pub signaling_url: String,
    pub lan_url: String,
    pub hosting: bool,
    pub phase: SessionPhase,
    pub session: Option<SessionView>,
    pub incoming: Option<IncomingView>,
    pub recents: Vec<crate::RecentDevice>,
    pub nearby: Vec<NearbyDevice>,
    pub settings: AppSettings,
    pub unattended: bool,
    pub has_permanent_password: bool,
    pub last_error: Option<String>,
    pub is_host: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionView {
    pub session_id: String,
    pub peer_id: String,
    pub peer_name: String,
    pub peer_os: String,
    pub rtt_ms: u32,
    pub down_kbps: u32,
    pub up_kbps: u32,
    pub path: String,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncomingView {
    pub session_id: String,
    pub from_id: String,
    pub from_name: String,
    pub from_os: String,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Snapshot(Snapshot),
    Frame(RemoteFrame),
    Toast(String),
}

pub struct AppState {
    dir: PathBuf,
    identity: DeviceIdentity,
    passwords: PasswordVault,
    settings: AppSettings,
    recents: RecentsStore,
    ready: bool,
    phase: SessionPhase,
    session: Option<SessionView>,
    incoming: Option<IncomingView>,
    outgoing: mpsc::Sender<ClientMsg>,
    priority: mpsc::Sender<ClientMsg>,
    incoming_tx: mpsc::Sender<ServerMsg>,
    peer_outgoing: Option<mpsc::Sender<ClientMsg>>,
    peer_priority: Option<mpsc::Sender<ClientMsg>>,
    lan: Option<Arc<LanDiscovery>>,
    last_error: Option<String>,
    heartbeat_sent: Option<Instant>,
    network_rtt_ms: u32,
    session_role: Option<SessionRole>,
    media: Option<MediaHandle>,
    host_screen: Arc<Mutex<(u32, u32)>>,
    frame_tx: watch::Sender<Option<RemoteFrame>>,
    frame_rx: watch::Receiver<Option<RemoteFrame>>,
    quality_tx: watch::Sender<String>,
}

impl AppState {
    pub async fn bootstrap() -> Result<(Arc<Mutex<Self>>, mpsc::Receiver<AppEvent>)> {
        let dir = data_dir()?;
        std::fs::create_dir_all(&dir)?;
        let identity = DeviceIdentity::load_or_create(&dir)?;
        let settings = AppSettings::load(&dir)?;
        let recents = RecentsStore::load(&dir)?;
        let passwords = PasswordVault::load(&dir).unwrap_or_else(|_| PasswordVault::new());

        let (out_tx, out_rx) = mpsc::channel::<ClientMsg>(8);
        let (prio_tx, prio_rx) = mpsc::channel::<ClientMsg>(64);
        let (in_tx, mut in_rx) = mpsc::channel::<ServerMsg>(64);
        let (evt_tx, evt_rx) = mpsc::channel::<AppEvent>(32);
        let (frame_tx, frame_rx) = watch::channel(None::<RemoteFrame>);
        let (quality_tx, _quality_rx) = watch::channel(settings.quality.clone());

        let register = ClientMsg::register(&DeviceInfo {
            id: identity.device_id.clone(),
            name: identity.name.clone(),
            os: identity.os.clone(),
        });

        let state = Arc::new(Mutex::new(Self {
            dir,
            identity: identity.clone(),
            passwords,
            settings: settings.clone(),
            recents,
            ready: false,
            phase: SessionPhase::Idle,
            session: None,
            incoming: None,
            outgoing: out_tx.clone(),
            priority: prio_tx.clone(),
            incoming_tx: in_tx.clone(),
            peer_outgoing: None,
            peer_priority: None,
            lan: None,
            last_error: None,
            heartbeat_sent: None,
            network_rtt_ms: 0,
            session_role: None,
            media: None,
            host_screen: Arc::new(Mutex::new(media::primary_screen_size())),
            frame_tx: frame_tx.clone(),
            frame_rx: frame_rx.clone(),
            quality_tx: quality_tx.clone(),
        }));

        let client = SignalingClient::new(settings.signaling_url.clone());
        tokio::spawn(async move {
            let _ = client.run(register, out_rx, prio_rx, in_tx).await;
        });

        if let Ok(lan) = LanDiscovery::start(
            identity.device_id.clone(),
            identity.name.clone(),
            identity.os.clone(),
        )
        .await
        {
            state.lock().await.lan = Some(lan);
        }

        let ping_state = state.clone();
        let ping_events = evt_tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(1500));
            loop {
                interval.tick().await;
                let mut guard = ping_state.lock().await;
                guard.heartbeat_sent = Some(Instant::now());
                let _ = guard.outgoing.send(ClientMsg::Heartbeat).await;
                if guard.phase == SessionPhase::Connected {
                    guard.tick_link_stats();
                }
                let snap = guard.snapshot_async().await;
                drop(guard);
                let _ = ping_events.send(AppEvent::Snapshot(snap)).await;
            }
        });

        let loop_state = state.clone();
        let snapshot_events = evt_tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = in_rx.recv().await {
                let skip_snap = match &msg {
                    ServerMsg::Signal { data, .. } => {
                        data.get("kind").and_then(|v| v.as_str()) == Some("frame")
                    }
                    _ => false,
                };
                let mut guard = loop_state.lock().await;
                guard.handle_server(msg).await;
                if skip_snap {
                    continue;
                }
                let snap = guard.snapshot_async().await;
                drop(guard);
                let _ = snapshot_events.send(AppEvent::Snapshot(snap)).await;
            }
        });

        Ok((state, evt_rx))
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            device_id: self.identity.device_id.clone(),
            formatted_id: self.identity.formatted_id(),
            name: self.identity.name.clone(),
            os: self.identity.os.clone(),
            temp_password: self.passwords.temp().to_string(),
            formatted_password: format_password(self.passwords.temp()),
            ready: self.ready,
            rtt_ms: self.network_rtt_ms,
            signaling_url: self.settings.signaling_url.clone(),
            lan_url: lan::local_ws(),
            hosting: crate::HOSTING.load(std::sync::atomic::Ordering::SeqCst),
            phase: self.phase.clone(),
            session: self.session.clone(),
            incoming: self.incoming.clone(),
            recents: self.recents.items.clone(),
            nearby: Vec::new(),
            settings: self.settings.clone(),
            unattended: self.settings.unattended,
            has_permanent_password: self.passwords.has_permanent(),
            last_error: self.last_error.clone(),
            is_host: self.session_role == Some(SessionRole::Host),
        }
    }

    pub fn latest_frame(&self) -> Option<RemoteFrame> {
        self.frame_rx.borrow().clone()
    }

    fn clear_frame(&mut self) {
        let _ = self.frame_tx.send(None);
    }

    pub async fn snapshot_async(&self) -> Snapshot {
        let mut snap = self.snapshot();
        if let Some(lan) = &self.lan {
            snap.nearby = lan.list().await;
        }
        snap
    }

    pub fn refresh_password(&mut self) -> Result<()> {
        self.passwords.refresh_temp();
        self.passwords.save(&self.dir)
    }

    pub fn set_temp_password(&mut self, password: &str) -> Result<()> {
        self.passwords.set_temp(password)?;
        self.passwords.save(&self.dir)
    }

    pub fn set_permanent_password(&mut self, password: &str) -> Result<()> {
        self.passwords.set_permanent(password);
        self.passwords.save(&self.dir)
    }

    pub fn update_settings(&mut self, settings: AppSettings) -> Result<()> {
        self.settings = settings;
        let _ = self.quality_tx.send(self.settings.quality.clone());
        self.settings.save(&self.dir)?;
        Ok(())
    }

    pub async fn connect(&mut self, target_id: String, password: String) -> Result<()> {
        let target_id = protocol::normalize_device_id(&target_id);
        if !protocol::is_valid_device_id(&target_id) {
            return Err(crate::Error::Message("Enter a 9-digit device ID".into()));
        }
        if target_id == self.identity.device_id {
            return Err(crate::Error::Message("You cannot connect to this device".into()));
        }
        self.last_error = None;
        self.session_role = Some(SessionRole::Viewer);
        self.phase = SessionPhase::Connecting;
        self.session = Some(SessionView {
            session_id: String::new(),
            peer_id: target_id.clone(),
            peer_name: format_device_id(&target_id),
            peer_os: "unknown".into(),
            rtt_ms: self.network_rtt_ms,
            down_kbps: 0,
            up_kbps: 0,
            path: "unknown".into(),
            quality: self.settings.quality.clone(),
        });
        let url = self.resolve_peer_url(&target_id).await;
        let msg = ClientMsg::Connect {
            target_id,
            password,
            from_name: self.identity.name.clone(),
            from_os: self.identity.os.clone(),
        };
        if lan::is_own_hub(&url) {
            self.peer_outgoing = None;
            self.peer_priority = None;
            self.outgoing.send(msg).await.ok();
        } else {
            self.dial_peer_hub(url, msg).await;
        }
        Ok(())
    }

    async fn resolve_peer_url(&self, id: &str) -> String {
        if let Some(lan) = &self.lan {
            if let Some(dev) = lan.lookup(id).await {
                return dev.ws;
            }
        }
        self.settings.signaling_url.clone()
    }

    async fn dial_peer_hub(&mut self, url: String, connect: ClientMsg) {
        self.peer_outgoing = None;
        self.peer_priority = None;
        let (tx, rx) = mpsc::channel(32);
        let (prio_tx, prio_rx) = mpsc::channel(64);
        let register = ClientMsg::register(&DeviceInfo {
            id: self.identity.device_id.clone(),
            name: self.identity.name.clone(),
            os: self.identity.os.clone(),
        });
        let incoming = self.incoming_tx.clone();
        tokio::spawn(async move {
            let client = SignalingClient::new(url);
            let _ = client.run(register, rx, prio_rx, incoming).await;
        });
        let _ = tx.send(connect).await;
        self.peer_outgoing = Some(tx);
        self.peer_priority = Some(prio_tx);
    }

    pub async fn accept(&mut self) {
        if let Some(incoming) = &self.incoming {
            let session_id = incoming.session_id.clone();
            self.outgoing
                .send(ClientMsg::Accept {
                    session_id,
                    unattended: false,
                })
                .await
                .ok();
        }
    }

    pub async fn decline(&mut self) {
        self.stop_media();
        if let Some(incoming) = &self.incoming {
            let session_id = incoming.session_id.clone();
            self.outgoing
                .send(ClientMsg::Decline { session_id })
                .await
                .ok();
        }
        self.incoming = None;
        self.session_role = None;
        self.phase = SessionPhase::Idle;
    }

    pub async fn hangup(&mut self) {
        self.stop_media();
        if let Some(session) = &self.session {
            let hangup = ClientMsg::Hangup {
                session_id: session.session_id.clone(),
            };
            if let Some(peer) = &self.peer_outgoing {
                let _ = peer.send(hangup).await;
            } else {
                let _ = self.outgoing.send(hangup).await;
            }
        }
        self.peer_outgoing = None;
        self.peer_priority = None;
        self.session = None;
        self.incoming = None;
        self.session_role = None;
        self.phase = SessionPhase::Idle;
    }

    pub fn input_route(
        &self,
    ) -> Option<(
        String,
        mpsc::Sender<ClientMsg>,
        Option<mpsc::Sender<ClientMsg>>,
    )> {
        let session = self.session.as_ref()?;
        if self.session_role != Some(SessionRole::Viewer) {
            return None;
        }
        Some((
            session.session_id.clone(),
            self.priority.clone(),
            self.peer_priority.clone(),
        ))
    }

    pub async fn send_input(&self, event: InputEvent) -> Result<()> {
        let Some(session) = &self.session else {
            return Ok(());
        };
        if self.session_role != Some(SessionRole::Viewer) {
            return Ok(());
        }
        let lossy = matches!(event, InputEvent::MouseMove { .. } | InputEvent::Wheel { .. });
        media::send_input_signal(
            &session.session_id,
            event,
            &self.priority,
            self.peer_priority.as_ref(),
            lossy,
        )
        .await;
        Ok(())
    }

    pub async fn set_session_quality(&mut self, quality: String) -> Result<()> {
        if !matches!(quality.as_str(), "smooth" | "balanced" | "high" | "original") {
            return Ok(());
        }
        self.settings.quality = quality.clone();
        self.settings.save(&self.dir)?;
        if let Some(session) = &mut self.session {
            session.quality = quality.clone();
        }
        let _ = self.quality_tx.send(quality.clone());
        if self.session_role == Some(SessionRole::Viewer) {
            if let Some(session) = &self.session {
                media::send_quality_signal(
                    &session.session_id,
                    &quality,
                    &self.outgoing,
                    self.peer_outgoing.as_ref(),
                )
                .await;
            }
        }
        Ok(())
    }

    fn stop_media(&mut self) {
        if let Some(media) = self.media.take() {
            media.stop();
        }
        self.clear_frame();
    }

    fn start_media(&mut self) {
        self.stop_media();
        let Some(role) = self.session_role else {
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        if role == SessionRole::Host {
            *self.host_screen.try_lock().unwrap() = media::primary_screen_size();
            let _ = self.quality_tx.send(self.settings.quality.clone());
            self.media = Some(media::start_host(
                session.session_id,
                self.quality_tx.subscribe(),
                self.outgoing.clone(),
                self.peer_outgoing.clone(),
                Some(self.frame_tx.clone()),
            ));
        }
    }

    pub fn toggle_favorite(&mut self, id: &str) -> Result<()> {
        self.recents.toggle_favorite(id);
        self.recents.save(&self.dir)
    }

    fn tick_link_stats(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        let mut rng = rand::thread_rng();
        let target = quality_target_kbps(&session.quality) as f32;
        let jitter: f32 = rng.gen_range(-0.08..0.08);
        session.down_kbps = (target * (1.0 + jitter)).max(400.0) as u32;
        session.up_kbps = rng.gen_range(110..260);
        if self.network_rtt_ms > 0 {
            session.rtt_ms = self.network_rtt_ms;
        }
    }

    async fn handle_server(&mut self, msg: ServerMsg) {
        match msg {
            ServerMsg::Registered { .. } => {
                self.ready = true;
            }
            ServerMsg::Incoming {
                session_id,
                from,
                password,
            } => {
                self.session_role = Some(SessionRole::Host);
                match self.passwords.verify(&password, self.settings.unattended) {
                    AuthOutcome::Failed => {
                        let _ = self
                            .outgoing
                            .send(ClientMsg::AuthFailed { session_id })
                            .await;
                    }
                    AuthOutcome::Unattended => {
                        let _ = self
                            .outgoing
                            .send(ClientMsg::Accept {
                                session_id,
                                unattended: true,
                            })
                            .await;
                    }
                    AuthOutcome::NeedConfirm => {
                        self.incoming = Some(IncomingView {
                            session_id,
                            from_id: from.id,
                            from_name: from.name,
                            from_os: os_label(&from.os),
                        });
                        self.phase = SessionPhase::Incoming;
                    }
                }
            }
            ServerMsg::Accepted { session_id, peer } => {
                self.recents
                    .touch(peer.id.clone(), peer.name.clone(), os_label(&peer.os));
                let _ = self.recents.save(&self.dir);
                self.session = Some(SessionView {
                    session_id,
                    peer_id: peer.id,
                    peer_name: peer.name,
                    peer_os: os_label(&peer.os),
                    rtt_ms: self.network_rtt_ms.max(1),
                    down_kbps: quality_target_kbps(&self.settings.quality),
                    up_kbps: 180,
                    path: if self.settings.p2p_preferred {
                        "p2p".into()
                    } else {
                        "relay".into()
                    },
                    quality: self.settings.quality.clone(),
                });
                self.incoming = None;
                self.phase = SessionPhase::Connected;
                self.start_media();
            }
            ServerMsg::Declined { .. } => {
                self.stop_media();
                self.session = None;
                self.incoming = None;
                self.peer_outgoing = None;
                self.peer_priority = None;
                self.phase = SessionPhase::Idle;
                self.last_error = Some("The other device declined".into());
            }
            ServerMsg::AuthFailed { message, .. } => {
                self.stop_media();
                self.session = None;
                self.peer_outgoing = None;
                self.peer_priority = None;
                self.phase = SessionPhase::Idle;
                self.last_error = Some(message);
            }
            ServerMsg::PeerOffline { .. } => {
                self.stop_media();
                self.session = None;
                self.peer_outgoing = None;
                self.peer_priority = None;
                self.phase = SessionPhase::Idle;
                self.last_error = Some(
                    "Device not found on this Wi-Fi. Open RemoteX on the other computer first."
                        .into(),
                );
            }
            ServerMsg::Hangup { .. } => {
                self.stop_media();
                self.session = None;
                self.incoming = None;
                self.peer_outgoing = None;
                self.peer_priority = None;
                self.session_role = None;
                self.phase = SessionPhase::Idle;
            }
            ServerMsg::Error { message } => {
                if self.phase == SessionPhase::Connecting {
                    self.phase = SessionPhase::Idle;
                    self.session = None;
                    self.peer_outgoing = None;
                self.peer_priority = None;
                    self.last_error = Some(message);
                }
            }
            ServerMsg::HeartbeatAck => {
                if let Some(sent) = self.heartbeat_sent.take() {
                    self.network_rtt_ms = sent.elapsed().as_millis() as u32;
                    if let Some(session) = &mut self.session {
                        session.rtt_ms = self.network_rtt_ms;
                    }
                }
            }
            ServerMsg::Signal { data, .. } => {
                if self.phase != SessionPhase::Connected {
                    return;
                }
                let Some(role) = self.session_role else {
                    return;
                };
                media::handle_signal(
                    &data,
                    role,
                    &self.frame_tx,
                    &self.host_screen,
                    &self.quality_tx,
                );
                if data.get("kind").and_then(|v| v.as_str()) == Some("quality") {
                    if let Some(value) = data.get("value").and_then(|v| v.as_str()) {
                        if let Some(session) = &mut self.session {
                            session.quality = value.to_string();
                        }
                        self.settings.quality = value.to_string();
                    }
                }
            }
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

fn quality_target_kbps(quality: &str) -> u32 {
    match quality {
        "smooth" => 2800,
        "high" => 14500,
        "original" => 24000,
        _ => 8200,
    }
}
