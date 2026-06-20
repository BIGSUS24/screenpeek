use ss_core::config::Config;
use ss_core::Result;
use ss_stream::StreamSession;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::RwLock;

pub struct HttpServer {
    config: Config,
    session: Arc<RwLock<StreamSession>>,
}

impl HttpServer {
    pub fn new(config: Config, session: Arc<RwLock<StreamSession>>) -> Self {
        Self { config, session }
    }

    pub async fn start(&self) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.config.server.http_port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| ss_core::Error::Server(format!("Failed to bind HTTP server: {}", e)))?;

        tracing::info!("HTTP server listening on {}", addr);

        loop {
            let (stream, _) = listener
                .accept()
                .await
                .map_err(|e| ss_core::Error::Server(format!("Failed to accept connection: {}", e)))?;

            let config = self.config.clone();
            let session = self.session.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(stream, config, session).await {
                    tracing::error!("Connection error: {}", e);
                }
            });
        }
    }

    async fn handle_connection(
        stream: tokio::net::TcpStream,
        config: Config,
        session: Arc<RwLock<StreamSession>>,
    ) -> Result<()> {
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).await?;

        let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
        if parts.len() < 2 {
            return Err(ss_core::Error::Server("Invalid request".to_string()));
        }

        let method = parts[0];
        let raw_path = parts[1];
        let (path, _query) = raw_path.split_once('?').unwrap_or((raw_path, ""));

        match path {
            "/" | "/index.html" => {
                let html = include_str!("../../../viewer/index.html");
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                    html.len(),
                    html
                );
                reader.get_mut().write_all(response.as_bytes()).await?;
            }
            "/api/token" => {
                if method == "POST" {
                    let mut content_length: usize = 0;

                    loop {
                        let mut line = String::new();
                        reader.read_line(&mut line).await?;
                        if line.trim().is_empty() {
                            break;
                        }
                        if line.to_lowercase().starts_with("content-length:") {
                            content_length = line
                                .split(':')
                                .nth(1)
                                .unwrap_or("0")
                                .trim()
                                .parse()
                                .unwrap_or(0);
                        }
                    }

                    let mut buffer = vec![0u8; content_length];
                    if content_length > 0 {
                        tokio::io::AsyncReadExt::read_exact(&mut reader, &mut buffer).await?;
                    }

                    let token_manager = ss_core::TokenManager::new(&config)?;
                    let device_id = config.device.id.as_str();
                    let token = token_manager.create_token("browser_viewer", device_id)?;

                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{}",
                        serde_json::json!({ "token": token })
                    );
                    reader.get_mut().write_all(response.as_bytes()).await?;
                } else {
                    let response = "HTTP/1.1 405 Method Not Allowed\r\nAllow: POST\r\n\r\n";
                    reader.get_mut().write_all(response.as_bytes()).await?;
                }
            }
            "/api/status" => {
                let session_guard = session.read().await;
                let viewer_count = session_guard.peer_count().await;
                let max_viewers = session_guard.max_viewers();
                let status = serde_json::json!({
                    "viewers": viewer_count,
                    "max_viewers": max_viewers,
                    "device_id": config.device.id,
                });
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{}",
                    status
                );
                reader.get_mut().write_all(response.as_bytes()).await?;
            }
            "/stream" => {
                // No token required - the stream is open to anyone who can reach this port.
                // No screen capture available (e.g. running as a Session 0 service).
                if !session.read().await.capture_available().await {
                    let response = "HTTP/1.1 503 Service Unavailable\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nScreen capture is not available on this host. Run ss-service.exe --console in your desktop session.";
                    reader.get_mut().write_all(response.as_bytes()).await?;
                    return Ok(());
                }

                let mut rx = {
                    let guard = session.read().await;
                    guard.subscribe_frames()
                };

                // Wait briefly for the first real frame before committing to a 200.
                // When the service is up but the capture agent hasn't produced a
                // frame yet (just started / between logon and capture), returning
                // 503 lets the viewer retry instead of hanging on an empty stream.
                let first = loop {
                    match tokio::time::timeout(std::time::Duration::from_secs(4), rx.recv()).await {
                        Ok(Ok(jpeg)) => break Some(jpeg),
                        Ok(Err(RecvError::Lagged(_))) => continue,
                        Ok(Err(RecvError::Closed)) => break None,
                        Err(_) => break None, // timed out waiting for a frame
                    }
                };

                let first = match first {
                    Some(j) => j,
                    None => {
                        let response = "HTTP/1.1 503 Service Unavailable\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nNo frames yet - capture agent not producing. Retry shortly.";
                        reader.get_mut().write_all(response.as_bytes()).await?;
                        return Ok(());
                    }
                };

                let header = "HTTP/1.1 200 OK\r\n\
                    Content-Type: multipart/x-mixed-replace; boundary=ssframe\r\n\
                    Cache-Control: no-cache, no-store, must-revalidate\r\n\
                    Pragma: no-cache\r\n\
                    Connection: close\r\n\r\n";
                let stream = reader.get_mut();
                stream.write_all(header.as_bytes()).await?;

                tracing::info!("Viewer connected to MJPEG stream");
                let mut next = Some(first);
                loop {
                    let jpeg = match next.take() {
                        Some(j) => j,
                        None => match rx.recv().await {
                            Ok(j) => j,
                            // Slow client fell behind - skip dropped frames.
                            Err(RecvError::Lagged(_)) => continue,
                            Err(RecvError::Closed) => break,
                        },
                    };
                    let part = format!(
                        "--ssframe\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                        jpeg.len()
                    );
                    if stream.write_all(part.as_bytes()).await.is_err() {
                        break;
                    }
                    if stream.write_all(&jpeg).await.is_err() {
                        break;
                    }
                    if stream.write_all(b"\r\n").await.is_err() {
                        break;
                    }
                }
                tracing::info!("Viewer disconnected from MJPEG stream");
            }
            _ => {
                let response = "HTTP/1.1 404 Not Found\r\n\r\n";
                reader.get_mut().write_all(response.as_bytes()).await?;
            }
        }

        Ok(())
    }
}
