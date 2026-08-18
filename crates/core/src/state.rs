use protocol::{format_device_id, ClientMsg, DeviceInfo, OsKind, ServerMsg};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch, Mutex};

use crate::clipboard;
use crate::identity::{data_dir, format_password, DeviceIdentity};
use crate::lan::{self, LanDiscovery, NearbyDevice};
use crate::media::{self, CapturePrefs, MediaHandle, RemoteFrame, SessionRole, SignalSideEffect};
use crate::password::{AuthOutcome, PasswordVault};
use crate::recents::RecentsStore;
use crate::settings::{self, AppSettings, LinePick};
use crate::signaling::SignalingClient;
use crate::transfer::{self, TransferComplete, TransferHub};
use crate::Result;
use input::InputEvent;

struct ClipboardSyncState {
    suppress_until: Option<Instant>,
}

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
    pub active_line: String,
    pub line1_rtt_ms: u32,
    pub line2_rtt_ms: u32,
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
    FileReceived(TransferComplete),
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
    lan_outgoing: Option<mpsc::Sender<ClientMsg>>,
    lan_priority: Option<mpsc::Sender<ClientMsg>>,
    lan: Option<Arc<LanDiscovery>>,
    capture_prefs: Arc<Mutex<CapturePrefs>>,
    last_error: Option<String>,
    heartbeat_sent: Option<Instant>,
    network_rtt_ms: u32,
    session_role: Option<SessionRole>,
    media: Option<MediaHandle>,
    host_screen: Arc<Mutex<(u32, u32)>>,
    frame_tx: watch::Sender<Option<RemoteFrame>>,
    frame_rx: watch::Receiver<Option<RemoteFrame>>,
    quality_tx: watch::Sender<String>,
    signaling_url_tx: watch::Sender<String>,
    active_line: String,
    line1_rtt_ms: u32,
    line2_rtt_ms: u32,
    pending_connect: Option<ClientMsg>,
    connect_tried_fallback: bool,
    pending_used_lan: bool,
    connect_tried_public: bool,
    peer_link_id: u64,
    next_peer_link_id: u64,
    last_peer_alive: Option<Instant>,
    connect_started: Option<Instant>,
    events: mpsc::Sender<AppEvent>,
    transfer_hub: TransferHub,
    clipboard_sync: Arc<Mutex<ClipboardSyncState>>,
    clipboard_stop: Option<watch::Sender<bool>>,
    link_bytes: Arc<(AtomicU64, AtomicU64)>,
    link_stats_at: Instant,
}

impl AppState {
    pub async fn bootstrap() -> Result<(Arc<Mutex<Self>>, mpsc::Receiver<AppEvent>)> {
        let dir = data_dir()?;
        std::fs::create_dir_all(&dir)?;
        let identity = DeviceIdentity::load_or_create(&dir)?;
        let mut settings = AppSettings::load(&dir)?;
        let recents = RecentsStore::load(&dir)?;
        let passwords = PasswordVault::load(&dir).unwrap_or_else(|_| PasswordVault::new());
        let probe = settings::probe_best_line().await;
        let pick = if settings.is_auto() {
            probe
        } else {
            LinePick {
                line: settings.signaling_line.clone(),
                url: settings.signaling_url.clone(),
                line1_rtt_ms: probe.line1_rtt_ms,
                line2_rtt_ms: probe.line2_rtt_ms,
            }
        };
        settings.signaling_url = pick.url.clone();

        let (out_tx, out_rx) = mpsc::channel::<ClientMsg>(4);
        let (prio_tx, prio_rx) = mpsc::channel::<ClientMsg>(128);
        let (in_tx, mut in_rx) = mpsc::channel::<ServerMsg>(64);
        let (evt_tx, evt_rx) = mpsc::channel::<AppEvent>(64);
        let (frame_tx, frame_rx) = watch::channel(None::<RemoteFrame>);
        let (quality_tx, _quality_rx) = watch::channel(settings.quality.clone());
        let (signaling_url_tx, signaling_url_rx) = watch::channel(settings.signaling_url.clone());
        let (lan_out_tx, lan_out_rx) = mpsc::channel::<ClientMsg>(32);
        let (lan_prio_tx, lan_prio_rx) = mpsc::channel::<ClientMsg>(64);

        let register = ClientMsg::register(&DeviceInfo {
            id: identity.device_id.clone(),
            name: identity.name.clone(),
            os: identity.os.clone(),
        });
        let lan_register = register.clone();
        let lan_incoming = in_tx.clone();

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
            lan_outgoing: Some(lan_out_tx),
            lan_priority: Some(lan_prio_tx),
            lan: None,
            capture_prefs: Arc::new(Mutex::new(CapturePrefs {
                resolution: settings.resolution.clone(),
                fps: settings.fps,
                viewport_w: 1920,
            })),
            last_error: None,
            heartbeat_sent: None,
            network_rtt_ms: 0,
            session_role: None,
            media: None,
            host_screen: Arc::new(Mutex::new(media::primary_screen_size())),
            frame_tx: frame_tx.clone(),
            frame_rx: frame_rx.clone(),
            quality_tx: quality_tx.clone(),
            signaling_url_tx: signaling_url_tx.clone(),
            active_line: pick.line.clone(),
            line1_rtt_ms: pick.line1_rtt_ms,
            line2_rtt_ms: pick.line2_rtt_ms,
            pending_connect: None,
            connect_tried_fallback: false,
            pending_used_lan: false,
            connect_tried_public: false,
            peer_link_id: 0,
            next_peer_link_id: 1,
            last_peer_alive: None,
            connect_started: None,
            events: evt_tx.clone(),
            transfer_hub: TransferHub::default(),
            clipboard_sync: Arc::new(Mutex::new(ClipboardSyncState {
                suppress_until: None,
            })),
            clipboard_stop: None,
            link_bytes: Arc::new((AtomicU64::new(0), AtomicU64::new(0))),
            link_stats_at: Instant::now(),
        }));

        tokio::spawn(async move {
            let _ = SignalingClient::run_watching(
                signaling_url_rx,
                register,
                out_rx,
                prio_rx,
                in_tx,
            )
            .await;
        });

        tokio::spawn(async move {
            let client = SignalingClient::new("ws://127.0.0.1:7829/ws".into());
            let _ = client
                .run(lan_register, lan_out_rx, lan_prio_rx, lan_incoming)
                .await;
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
                let _ = guard.priority.try_send(ClientMsg::Heartbeat);
                if let Some(lan) = &guard.lan_priority {
                    let _ = lan.try_send(ClientMsg::Heartbeat);
                }
                if guard.phase == SessionPhase::Connecting {
                    if guard
                        .connect_started
                        .is_some_and(|at| at.elapsed() > Duration::from_secs(45))
                    {
                        guard.last_error = Some("Connection timed out".into());
                        guard.reset_session();
                    }
                } else if guard.phase == SessionPhase::Connected {
                    if guard
                        .last_peer_alive
                        .is_some_and(|at| at.elapsed() > Duration::from_secs(12))
                    {
                        let was_host = guard.session_role == Some(SessionRole::Host);
                        guard.last_error = Some("Remote device disconnected".into());
                        guard.finish_session(was_host);
                    } else {
                        guard.tick_link_stats();
                    }
                }
                let snap = if guard.phase == SessionPhase::Connected {
                    guard.snapshot()
                } else {
                    guard.snapshot_async().await
                };
                drop(guard);
                let _ = ping_events.send(AppEvent::Snapshot(snap)).await;
            }
        });

        let mut frame_events_rx = frame_rx.clone();
        let frame_events = evt_tx.clone();
        tokio::spawn(async move {
            loop {
                if frame_events_rx.changed().await.is_err() {
                    break;
                }
                if let Some(frame) = frame_events_rx.borrow_and_update().clone() {
                    let _ = frame_events.try_send(AppEvent::Frame(frame));
                }
            }
        });

        let loop_state = state.clone();
        let snapshot_events = evt_tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = in_rx.recv().await {
                let skip_snap = match &msg {
                    ServerMsg::Signal { data, .. } => matches!(
                        data.get("kind").and_then(|v| v.as_str()),
                        Some("frame") | Some("file_chunk") | Some("keepalive")
                    ),
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
            active_line: self.active_line.clone(),
            line1_rtt_ms: self.line1_rtt_ms,
            line2_rtt_ms: self.line2_rtt_ms,
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
        if self.phase != SessionPhase::Connected {
            if let Some(lan) = &self.lan {
                snap.nearby = lan.list().await;
            }
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
        self.passwords.set_permanent(password)?;
        self.passwords.save(&self.dir)
    }

    pub async fn update_settings(&mut self, mut settings: AppSettings) -> Result<()> {
        settings.normalize();
        let probe = settings::probe_best_line().await;
        self.line1_rtt_ms = probe.line1_rtt_ms;
        self.line2_rtt_ms = probe.line2_rtt_ms;
        if settings.is_auto() {
            settings.signaling_url = probe.url.clone();
            self.active_line = probe.line;
        } else {
            self.active_line = settings.signaling_line.clone();
        }
        let url_changed = settings.signaling_url != self.settings.signaling_url;
        self.settings = settings.clone();
        let _ = self.quality_tx.send(self.settings.quality.clone());
        {
            let mut prefs = self.capture_prefs.lock().await;
            prefs.resolution = self.settings.resolution.clone();
            prefs.fps = self.settings.fps;
        }
        if url_changed {
            self.ready = false;
            let _ = self
                .signaling_url_tx
                .send(self.settings.signaling_url.clone());
        }
        self.settings.save(&self.dir)?;
        if self.phase == SessionPhase::Connected {
            self.start_clipboard_sync();
        }
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
        self.connect_started = Some(Instant::now());
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
        let using_lan = lan::is_lan_url(&url)
            && url != settings::PUBLIC_SIGNALING_URL
            && url != settings::LINE2_SIGNALING_URL;
        let msg = ClientMsg::Connect {
            target_id,
            password,
            from_name: self.identity.name.clone(),
            from_os: self.identity.os.clone(),
        };
        self.pending_connect = Some(msg.clone());
        self.connect_tried_fallback = false;
        self.pending_used_lan = using_lan;
        self.connect_tried_public = !using_lan;
        if using_lan {
            if let Some(session) = &mut self.session {
                session.path = "lan".into();
            }
        }
        if lan::is_own_hub(&url) {
            self.peer_outgoing = None;
            self.peer_priority = None;
            if let Some(lan) = &self.lan_outgoing {
                lan.send(msg).await.ok();
            } else {
                self.outgoing.send(msg).await.ok();
            }
        } else {
            self.dial_peer_hub(url, msg).await;
        }
        Ok(())
    }

    async fn resolve_peer_url(&self, id: &str) -> String {
        if self.settings.prefer_lan {
            if let Some(lan) = &self.lan {
                if let Some(dev) = lan.lookup(id).await {
                    if lan::hub_reachable(&dev.ws).await {
                        return dev.ws;
                    }
                }
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
        self.next_peer_link_id = self.next_peer_link_id.wrapping_add(1);
        let link_id = self.next_peer_link_id;
        self.peer_link_id = link_id;
        tokio::spawn(async move {
            let client = SignalingClient::new(url);
            let _ = client.run_once(register, rx, prio_rx, incoming.clone()).await;
            let _ = incoming
                .send(ServerMsg::Hangup {
                    session_id: format!("link:{link_id}"),
                })
                .await;
        });
        let _ = tx.send(connect).await;
        self.peer_outgoing = Some(tx);
        self.peer_priority = Some(prio_tx);
    }

    async fn send_control(&self, msg: ClientMsg) {
        let _ = self.outgoing.send(msg.clone()).await;
        if let Some(lan) = &self.lan_outgoing {
            let _ = lan.send(msg.clone()).await;
        }
        if let Some(peer) = &self.peer_outgoing {
            let _ = peer.send(msg).await;
        }
    }

    pub async fn accept(&mut self) {
        if let Some(incoming) = &self.incoming {
            let session_id = incoming.session_id.clone();
            self.send_control(ClientMsg::Accept {
                session_id,
                unattended: false,
            })
            .await;
        }
    }

    pub async fn decline(&mut self) {
        if let Some(incoming) = &self.incoming {
            let session_id = incoming.session_id.clone();
            self.send_control(ClientMsg::Decline { session_id }).await;
        }
        self.reset_session();
    }

    pub async fn hangup(&mut self) {
        let was_host = self.session_role == Some(SessionRole::Host);
        if let Some(session) = &self.session {
            if !session.session_id.is_empty() {
                self.send_control(ClientMsg::Hangup {
                    session_id: session.session_id.clone(),
                })
                .await;
            }
        }
        if self.phase == SessionPhase::Connecting {
            self.peer_outgoing = None;
            self.peer_priority = None;
        }
        self.finish_session(was_host);
    }

    pub async fn cancel_connect(&mut self) {
        if self.phase != SessionPhase::Connecting {
            return;
        }
        self.peer_outgoing = None;
        self.peer_priority = None;
        self.reset_session();
    }

    fn reset_session(&mut self) {
        self.stop_media();
        self.session = None;
        self.incoming = None;
        self.peer_outgoing = None;
        self.peer_priority = None;
        self.pending_connect = None;
        self.connect_tried_fallback = false;
        self.pending_used_lan = false;
        self.connect_tried_public = false;
        self.peer_link_id = 0;
        self.last_peer_alive = None;
        self.connect_started = None;
        self.session_role = None;
        self.phase = SessionPhase::Idle;
        self.link_bytes.0.store(0, Ordering::Relaxed);
        self.link_bytes.1.store(0, Ordering::Relaxed);
        self.link_stats_at = Instant::now();
    }

    fn finish_session(&mut self, was_host: bool) {
        self.reset_session();
        if was_host && self.settings.lock_after_session {
            crate::lock::lock_workstation();
        }
    }

    pub fn input_route(
        &self,
    ) -> Option<(
        String,
        mpsc::Sender<ClientMsg>,
        Option<mpsc::Sender<ClientMsg>>,
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
            self.lan_priority.clone(),
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
            self.lan_priority.as_ref(),
            lossy,
        )
        .await;
        Ok(())
    }

    pub async fn send_viewport(&self, width: u32, height: u32) -> Result<()> {
        let Some(session) = &self.session else {
            return Ok(());
        };
        if self.session_role != Some(SessionRole::Viewer) {
            return Ok(());
        }
        media::send_viewport_signal(
            &session.session_id,
            width,
            height,
            &self.outgoing,
            self.peer_outgoing.as_ref(),
            self.lan_outgoing.as_ref(),
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
                    self.lan_outgoing.as_ref(),
                )
                .await;
            }
        }
        Ok(())
    }

    pub async fn send_file(&self, path: &Path) -> Result<()> {
        if !self.settings.allow_file_transfer {
            return Err(crate::Error::Message("File transfer is disabled".into()));
        }
        if self.phase != SessionPhase::Connected {
            return Err(crate::Error::Message("No active session".into()));
        }
        let Some(session) = &self.session else {
            return Err(crate::Error::Message("No active session".into()));
        };
        transfer::send_file(
            &session.session_id,
            path,
            &self.outgoing,
            self.peer_outgoing.as_ref(),
            self.lan_outgoing.as_ref(),
        )
        .await
    }

    fn stop_clipboard_sync(&mut self) {
        if let Some(tx) = self.clipboard_stop.take() {
            let _ = tx.send(true);
        }
    }

    fn start_clipboard_sync(&mut self) {
        self.stop_clipboard_sync();
        if !self.settings.allow_clipboard {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        let (stop_tx, mut stop_rx) = watch::channel(false);
        self.clipboard_stop = Some(stop_tx);
        let outgoing = self.outgoing.clone();
        let peer = self.peer_outgoing.clone();
        let lan = self.lan_outgoing.clone();
        let sync = self.clipboard_sync.clone();
        tokio::spawn(async move {
            let mut last_sent = String::new();
            let mut interval = tokio::time::interval(Duration::from_millis(650));
            loop {
                tokio::select! {
                    res = stop_rx.changed() => {
                        if res.is_err() || *stop_rx.borrow() {
                            break;
                        }
                    }
                    _ = interval.tick() => {
                        let Some(text) = clipboard::read_text() else { continue };
                        let suppress = {
                            let guard = sync.lock().await;
                            guard
                                .suppress_until
                                .map(|t| Instant::now() < t)
                                .unwrap_or(false)
                        };
                        if suppress {
                            last_sent = text;
                            continue;
                        }
                        if text != last_sent && text.len() <= 512_000 {
                            media::send_clipboard_signal(
                                &session.session_id,
                                &text,
                                &outgoing,
                                peer.as_ref(),
                                lan.as_ref(),
                            )
                            .await;
                            last_sent = text;
                        }
                    }
                }
            }
        });
    }

    fn stop_media(&mut self) {
        self.stop_clipboard_sync();
        self.transfer_hub.reset();
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
                self.capture_prefs.clone(),
                self.outgoing.clone(),
                self.peer_outgoing.clone(),
                self.lan_outgoing.clone(),
                Some(self.frame_tx.clone()),
                self.host_screen.clone(),
                self.link_bytes.clone(),
            ));
        }
        self.start_clipboard_sync();
    }

    pub fn toggle_favorite(&mut self, id: &str) -> Result<()> {
        self.recents.toggle_favorite(id);
        self.recents.save(&self.dir)
    }

    fn tick_link_stats(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        let elapsed = self.link_stats_at.elapsed().as_secs_f64().max(0.75);
        let rx = self.link_bytes.0.swap(0, Ordering::Relaxed);
        let tx = self.link_bytes.1.swap(0, Ordering::Relaxed);
        self.link_stats_at = Instant::now();
        let kbps = |bytes: u64| ((bytes as f64 * 8.0 / 1000.0) / elapsed).round() as u32;
        match self.session_role {
            Some(SessionRole::Viewer) => {
                session.down_kbps = kbps(rx);
                session.up_kbps = kbps(tx);
            }
            Some(SessionRole::Host) => {
                session.up_kbps = kbps(tx);
                session.down_kbps = kbps(rx);
            }
            None => {}
        }
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
                if matches!(
                    self.phase,
                    SessionPhase::Connected | SessionPhase::Connecting | SessionPhase::Incoming
                ) {
                    let _ = self
                        .send_control(ClientMsg::Decline { session_id })
                        .await;
                    return;
                }
                match self.passwords.verify(&password, self.settings.unattended) {
                    AuthOutcome::Failed => {
                        self.send_control(ClientMsg::AuthFailed { session_id }).await;
                    }
                    AuthOutcome::Unattended => {
                        self.session_role = Some(SessionRole::Host);
                        self.send_control(ClientMsg::Accept {
                            session_id,
                            unattended: true,
                        })
                        .await;
                    }
                    AuthOutcome::NeedConfirm if !self.settings.require_confirm => {
                        self.session_role = Some(SessionRole::Host);
                        self.send_control(ClientMsg::Accept {
                            session_id,
                            unattended: false,
                        })
                        .await;
                    }
                    AuthOutcome::NeedConfirm => {
                        self.session_role = Some(SessionRole::Host);
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
                if self.phase != SessionPhase::Connecting
                    || self.session_role != Some(SessionRole::Viewer)
                {
                    self.send_control(ClientMsg::Hangup { session_id }).await;
                    return;
                }
                self.pending_connect = None;
                self.connect_tried_fallback = false;
                self.recents
                    .touch(peer.id.clone(), peer.name.clone(), os_label(&peer.os));
                let _ = self.recents.save(&self.dir);
                let path = if self.pending_used_lan { "lan" } else { "relay" };
                self.session = Some(SessionView {
                    session_id,
                    peer_id: peer.id,
                    peer_name: peer.name,
                    peer_os: os_label(&peer.os),
                    rtt_ms: self.network_rtt_ms.max(1),
                    down_kbps: 0,
                    up_kbps: 0,
                    path: path.into(),
                    quality: self.settings.quality.clone(),
                });
                self.incoming = None;
                self.phase = SessionPhase::Connected;
                self.last_peer_alive = Some(Instant::now());
                self.start_media();
            }
            ServerMsg::Declined { .. } => {
                self.reset_session();
                self.last_error = Some("The other device declined".into());
            }
            ServerMsg::AuthFailed { message, .. } => {
                self.reset_session();
                self.last_error = Some(message);
            }
            ServerMsg::PeerOffline { .. } => {
                if self.phase == SessionPhase::Connecting {
                    if let Some(msg) = self.pending_connect.clone() {
                        if self.pending_used_lan && !self.connect_tried_public {
                            self.pending_used_lan = false;
                            self.connect_tried_public = true;
                            if let Some(session) = &mut self.session {
                                session.path = "relay".into();
                            }
                            let url = self.settings.signaling_url.clone();
                            if lan::is_own_hub(&url) {
                                self.peer_outgoing = None;
                                self.peer_priority = None;
                                self.outgoing.send(msg).await.ok();
                            } else {
                                self.dial_peer_hub(url, msg).await;
                            }
                            return;
                        }
                        if self.settings.is_auto() && !self.connect_tried_fallback {
                            self.connect_tried_fallback = true;
                            let other = settings::other_line_url(&self.settings.signaling_url);
                            self.settings.signaling_url = other.to_string();
                            self.active_line = settings::line_of_url(other).into();
                            let _ = self.signaling_url_tx.send(self.settings.signaling_url.clone());
                            self.dial_peer_hub(other.to_string(), msg).await;
                            return;
                        }
                    }
                }
                self.reset_session();
                self.last_error = Some(
                    "Device not found on this Wi-Fi. Open RemoteX on the other computer first."
                        .into(),
                );
            }
            ServerMsg::Hangup { session_id } => {
                if let Some(rest) = session_id.strip_prefix("link:") {
                    if rest.parse::<u64>().ok() != Some(self.peer_link_id) || self.peer_link_id == 0 {
                        return;
                    }
                }
                let was_host = self.session_role == Some(SessionRole::Host);
                if self.phase == SessionPhase::Connected {
                    self.last_error = Some("Remote device disconnected".into());
                }
                self.finish_session(was_host);
            }
            ServerMsg::Error { message } => {
                if self.phase == SessionPhase::Connecting {
                    self.reset_session();
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
                self.last_peer_alive = Some(Instant::now());
                let Some(role) = self.session_role else {
                    return;
                };
                match media::handle_signal(
                    &data,
                    role,
                    &self.frame_tx,
                    &self.host_screen,
                    &self.quality_tx,
                    self.settings.allow_clipboard,
                    self.settings.allow_file_transfer,
                    &mut self.transfer_hub,
                    Some(&self.link_bytes.0),
                    Some(&self.link_bytes.1),
                ) {
                    Ok(Some(SignalSideEffect::ClipboardSynced(_))) => {
                        let mut sync = self.clipboard_sync.lock().await;
                        sync.suppress_until = Some(Instant::now() + Duration::from_secs(2));
                    }
                    Ok(Some(SignalSideEffect::FileReceived(done))) => {
                        let _ = self
                            .events
                            .send(AppEvent::FileReceived(done))
                            .await;
                    }
                    Ok(Some(SignalSideEffect::Viewport(width))) => {
                        let mut prefs = self.capture_prefs.lock().await;
                        prefs.viewport_w = width;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        self.last_error = Some(e.to_string());
                    }
                }
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
