use futures_util::{SinkExt, StreamExt};
use protocol::{ClientMsg, ServerMsg};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::{connect_async_with_config, tungstenite::Message};
use tracing::{info, warn};

use crate::{Error, Result};

pub struct SignalingClient {
    pub url: String,
}

impl SignalingClient {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    pub async fn run(
        &self,
        register: ClientMsg,
        outgoing: mpsc::Receiver<ClientMsg>,
        priority: mpsc::Receiver<ClientMsg>,
        incoming: mpsc::Sender<ServerMsg>,
    ) -> Result<()> {
        let (_tx, rx) = watch::channel(self.url.clone());
        Self::run_watching(rx, register, outgoing, priority, incoming).await
    }

    pub async fn run_watching(
        mut url_rx: watch::Receiver<String>,
        register: ClientMsg,
        mut outgoing: mpsc::Receiver<ClientMsg>,
        mut priority: mpsc::Receiver<ClientMsg>,
        incoming: mpsc::Sender<ServerMsg>,
    ) -> Result<()> {
        loop {
            let url = url_rx.borrow().clone();
            match connect_once(
                &url,
                &register,
                &mut outgoing,
                &mut priority,
                &incoming,
                &mut url_rx,
            )
            .await
            {
                Ok(ReconnectReason::UrlChanged) => {
                    info!("signaling url changed, reconnecting");
                }
                Ok(ReconnectReason::Closed) => info!("signaling socket closed"),
                Err(err) => {
                    warn!("signaling error: {err}");
                    if outgoing.is_closed() {
                        return Ok(());
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                    continue;
                }
            }
            if outgoing.is_closed() {
                return Ok(());
            }
        }
    }
}

enum ReconnectReason {
    Closed,
    UrlChanged,
}

async fn connect_once(
    url: &str,
    register: &ClientMsg,
    outgoing: &mut mpsc::Receiver<ClientMsg>,
    priority: &mut mpsc::Receiver<ClientMsg>,
    incoming: &mpsc::Sender<ServerMsg>,
    url_rx: &mut watch::Receiver<String>,
) -> Result<ReconnectReason> {
    let _parsed = url::Url::parse(url)
        .map_err(|e| Error::Message(format!("invalid signaling url: {e}")))?;
    let config = WebSocketConfig::default()
        .max_message_size(Some(16 * 1024 * 1024))
        .max_frame_size(Some(16 * 1024 * 1024));
    let (ws, _) = connect_async_with_config(url.to_string(), Some(config), false).await?;
    let (mut sink, mut stream) = ws.split();
    info!("connected to signaling {url}");
    let json = serde_json::to_string(register)?;
    sink.send(Message::Text(json.into())).await?;

    loop {
        tokio::select! {
            biased;
            changed = url_rx.changed() => {
                if changed.is_err() {
                    return Ok(ReconnectReason::Closed);
                }
                return Ok(ReconnectReason::UrlChanged);
            }
            msg = priority.recv() => {
                let Some(msg) = msg else { continue; };
                let json = serde_json::to_string(&msg)?;
                sink.send(Message::Text(json.into())).await?;
            }
            msg = outgoing.recv() => {
                let Some(msg) = msg else { return Ok(ReconnectReason::Closed); };
                let json = serde_json::to_string(&msg)?;
                sink.send(Message::Text(json.into())).await?;
            }
            frame = stream.next() => {
                match frame {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(msg) = serde_json::from_str::<ServerMsg>(&text) {
                            let _ = incoming.send(msg).await;
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        sink.send(Message::Pong(data)).await?;
                    }
                    Some(Ok(Message::Close(_))) | None => return Ok(ReconnectReason::Closed),
                    Some(Err(err)) => return Err(err.into()),
                    _ => {}
                }
            }
        }
    }
}
