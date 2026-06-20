use ss_core::config::Config;
use ss_core::Result;
use ss_stream::peer::{IceCandidate, SdpAnswer, SdpOffer};
use ss_stream::{PeerConnection, StreamSession};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

pub struct SignalServer {
    config: Config,
    session: Arc<RwLock<StreamSession>>,
}

impl SignalServer {
    pub fn new(config: Config, session: Arc<RwLock<StreamSession>>) -> Self {
        Self { config, session }
    }

    pub async fn start(&self) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.config.server.https_port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| ss_core::Error::Server(format!("Failed to bind signaling server: {}", e)))?;

        tracing::info!("Signaling server listening on {}", addr);

        loop {
            let (stream, _) = listener
                .accept()
                .await
                .map_err(|e| ss_core::Error::Server(format!("Failed to accept connection: {}", e)))?;

            let config = self.config.clone();
            let session = self.session.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(stream, config, session).await {
                    tracing::error!("Signaling error: {}", e);
                }
            });
        }
    }

    async fn handle_connection(
        stream: tokio::net::TcpStream,
        config: Config,
        session: Arc<RwLock<StreamSession>>,
    ) -> Result<()> {
        let ws_stream = tokio_tungstenite::accept_async(stream)
            .await
            .map_err(|e| ss_core::Error::Server(format!("WebSocket handshake failed: {}", e)))?;

        use futures_util::{SinkExt, StreamExt};
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        let mut current_peer_id: Option<String> = None;

        while let Some(msg) = ws_receiver.next().await {
            let msg = msg.map_err(|e| ss_core::Error::Server(format!("WebSocket error: {}", e)))?;

            if msg.is_text() {
                let text = msg.to_text().map_err(|e| ss_core::Error::Server(format!("WS text error: {}", e)))?;
                let message: serde_json::Value = serde_json::from_str(text)?;

                match message["type"].as_str() {
                    Some("auth") => {
                        let token = message["token"].as_str().unwrap_or("");
                        let token_manager = ss_core::TokenManager::new(&config)?;

                        match token_manager.verify_token(token) {
                            Ok(_token_data) => {
                                let response = serde_json::json!({
                                    "type": "auth_ok",
                                    "device_id": config.device.id,
                                });
                                ws_sender
                                    .send(tokio_tungstenite::tungstenite::Message::Text(
                                        response.to_string().into(),
                                    ))
                                    .await
                                    .map_err(|e| ss_core::Error::Server(e.to_string()))?;
                            }
                            Err(e) => {
                                let response = serde_json::json!({
                                    "type": "auth_error",
                                    "error": e.to_string(),
                                });
                                ws_sender
                                    .send(tokio_tungstenite::tungstenite::Message::Text(
                                        response.to_string().into(),
                                    ))
                                    .await
                                    .map_err(|e| ss_core::Error::Server(e.to_string()))?;
                            }
                        }
                    }
                    Some("offer") => {
                        let sdp = message["sdp"].as_str().unwrap_or("");
                        let sdp_type = message["sdp_type"].as_str().unwrap_or("");

                        let peer_id = uuid::Uuid::new_v4().to_string();
                        current_peer_id = Some(peer_id.clone());
                        let mut peer = PeerConnection::new(peer_id.clone(), "viewer".to_string());

                        let offer = SdpOffer {
                            sdp: sdp.to_string(),
                            sdp_type: sdp_type.to_string(),
                        };
                        peer.set_offer(offer);

                        let answer_sdp = format!(
                            "v=0\r\no=- {} {} IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\nm=video 9 UDP/TLS/RTP/SAVPF 96\r\na=rtpmap:96 H264/90000\r\n",
                            peer_id,
                            chrono::Utc::now().timestamp()
                        );

                        let answer = SdpAnswer {
                            sdp: answer_sdp,
                            sdp_type: "answer".to_string(),
                        };
                        peer.set_answer(answer.clone());

                        let mut session_guard = session.write().await;
                        session_guard.add_peer(peer_id.clone(), peer).await;

                        let response = serde_json::json!({
                            "type": "answer",
                            "sdp": answer.sdp,
                            "sdp_type": answer.sdp_type,
                        });
                        ws_sender
                            .send(tokio_tungstenite::tungstenite::Message::Text(
                                response.to_string().into(),
                            ))
                            .await
                            .map_err(|e| ss_core::Error::Server(e.to_string()))?;
                    }
                    Some("ice") => {
                        let candidate_str = message["candidate"].as_str().unwrap_or("");
                        let sdp_mid = message["sdp_mid"].as_str().unwrap_or("");
                        let sdp_m_line_index = message["sdp_m_line_index"].as_u64().unwrap_or(0);

                        if let Some(ref pid) = current_peer_id {
                            let ice_candidate = IceCandidate {
                                candidate: candidate_str.to_string(),
                                sdp_mid: sdp_mid.to_string(),
                                sdp_m_line_index: sdp_m_line_index as u32,
                            };
                            let session_guard = session.read().await;
                            session_guard.add_ice_candidate(pid, ice_candidate).await;
                            tracing::debug!("Stored ICE candidate for peer {}: {}", pid, candidate_str);
                        } else {
                            tracing::warn!("Received ICE candidate before offer from peer");
                        }
                    }
                    _ => {
                        tracing::warn!("Unknown message type: {:?}", message["type"]);
                    }
                }
            }
        }

        Ok(())
    }
}
