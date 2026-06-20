use crate::pipeline::Pipeline;
use crate::peer::PeerConnection;
use ss_core::config::Config;
use ss_core::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

pub struct StreamSession {
    config: Config,
    pipeline: Arc<RwLock<Option<Pipeline>>>,
    peers: Arc<RwLock<HashMap<String, PeerConnection>>>,
    frame_tx: broadcast::Sender<Vec<u8>>,
    /// True when frames are injected externally (by a capture agent over the
    /// frame-intake socket) rather than produced by a local in-process pipeline.
    external_frames: bool,
}

impl StreamSession {
    pub fn new(config: Config) -> Result<Self> {
        let (frame_tx, _) = broadcast::channel(10);

        // Capture initialization can fail (e.g. a Windows service running in
        // Session 0 has no access to the interactive desktop / DXGI). Treat this
        // as non-fatal so the web and signaling servers still start and surface a
        // clear message instead of the whole process exiting.
        let pipeline = match Pipeline::new(
            config.capture.fps,
            config.capture.monitor_index,
            config.encoder.bitrate,
            config.encoder.keyframe_interval,
        ) {
            Ok(p) => {
                tracing::info!("Screen capture initialized");
                Some(p)
            }
            Err(e) => {
                tracing::warn!(
                    "Screen capture unavailable ({}). Servers will start but no video will stream. \
                     If running as a Windows service, run interactively instead: ss-service.exe --console",
                    e
                );
                None
            }
        };

        Ok(Self {
            config,
            pipeline: Arc::new(RwLock::new(pipeline)),
            peers: Arc::new(RwLock::new(HashMap::new())),
            frame_tx,
            external_frames: false,
        })
    }

    /// Build a session with no local capture pipeline. Frames are pushed in from
    /// outside (the capture agent) via [`StreamSession::frame_sender`]. Used by
    /// the Windows service, whose capture runs in a separate agent process.
    pub fn new_headless(config: Config) -> Self {
        let (frame_tx, _) = broadcast::channel(10);
        Self {
            config,
            pipeline: Arc::new(RwLock::new(None)),
            peers: Arc::new(RwLock::new(HashMap::new())),
            frame_tx,
            external_frames: true,
        }
    }

    /// A sender for injecting externally-produced (already JPEG-encoded) frames.
    pub fn frame_sender(&self) -> broadcast::Sender<Vec<u8>> {
        self.frame_tx.clone()
    }

    /// Whether the stream can serve video: either a local pipeline initialized,
    /// or we are accepting externally-injected frames.
    pub async fn capture_available(&self) -> bool {
        self.external_frames || self.pipeline.read().await.is_some()
    }

    pub async fn start(&self) -> Result<()> {
        if self.pipeline.read().await.is_none() {
            tracing::warn!("Stream session started without capture - serving UI only");
            return Ok(());
        }

        let pipeline = self.pipeline.clone();
        let frame_tx = self.frame_tx.clone();

        tokio::spawn(async move {
            loop {
                {
                    let mut guard = pipeline.write().await;
                    if let Some(p) = guard.as_mut() {
                        match p.capture_and_encode() {
                            Ok(Some(frame)) => {
                                let _ = frame_tx.send(frame.data);
                            }
                            Ok(None) => {}
                            Err(e) => {
                                tracing::error!("Pipeline error: {}", e);
                            }
                        }
                    } else {
                        break;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(16)).await;
            }
        });

        tracing::info!("Stream session started");
        Ok(())
    }

    pub fn subscribe_frames(&self) -> broadcast::Receiver<Vec<u8>> {
        self.frame_tx.subscribe()
    }

    pub async fn add_ice_candidate(&self, peer_id: &str, candidate: crate::peer::IceCandidate) {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(peer_id) {
            peer.add_ice_candidate(candidate);
        }
    }

    pub async fn add_peer(&self, peer_id: String, peer: PeerConnection) {
        let mut peers = self.peers.write().await;
        peers.insert(peer_id, peer);
    }

    pub async fn remove_peer(&self, peer_id: &str) {
        let mut peers = self.peers.write().await;
        peers.remove(peer_id);
    }

    pub async fn peer_count(&self) -> usize {
        let peers = self.peers.read().await;
        peers.len()
    }

    pub fn max_viewers(&self) -> usize {
        self.config.server.max_viewers
    }
}
