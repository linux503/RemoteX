use futures_util::{SinkExt, StreamExt};
use protocol::{ClientMsg, ServerMsg};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
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
        mut outgoing: mpsc::Receiver<ClientMsg>,
        incoming: mpsc::Sender<ServerMsg>,
    ) -> Result<()> {
        loop {
            match self
                .connect_once(&register, &mut outgoing, &incoming)
                .await
            {
                Ok(()) => info!("signaling socket closed"),
                Err(err) => warn!("signaling error: {err}"),
            }
            tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        }
    }

    async fn connect_once(
        &self,
        register: &ClientMsg,
        outgoing: &mut mpsc::Receiver<ClientMsg>,
        incoming: &mpsc::Sender<ServerMsg>,
    ) -> Result<()> {
        let url = self.url.clone();
        let _parsed = url::Url::parse(&url)
            .map_err(|e| Error::Message(format!("invalid signaling url: {e}")))?;
        let (ws, _) = connect_async(url).await?;
        let (mut sink, mut stream) = ws.split();
        info!("connected to signaling {}", self.url);
        let json = serde_json::to_string(register)?;
        sink.send(Message::Text(json.into())).await?;

        loop {
            tokio::select! {
                msg = outgoing.recv() => {
                    let Some(msg) = msg else { return Ok(()); };
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
                        Some(Ok(Message::Close(_))) | None => return Ok(()),
                        Some(Err(err)) => return Err(err.into()),
                        _ => {}
                    }
                }
            }
        }
    }
}
