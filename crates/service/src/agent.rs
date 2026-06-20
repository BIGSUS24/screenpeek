//! Capture agent.
//!
//! Runs as a separate process launched by the service into the *active console
//! session* as SYSTEM (see `win::launch_agent_in_session`). Because it lives in
//! the interactive session and runs as SYSTEM, it can follow the secure
//! (Winlogon) desktop and so capture the login screen, lock screen and UAC
//! prompts - things a Session 0 service can never see itself.
//!
//! It captures frames, JPEG-encodes them, and streams them to the service over a
//! loopback TCP socket as length-prefixed messages: `[u32 big-endian len][jpeg]`.

use ss_capture::DesktopCapture;
use ss_core::config::Config;
use ss_encoder::JpegEncoder;
use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;

/// Loopback address the agent streams frames to and the service listens on.
pub const FRAME_INTAKE_ADDR: &str = "127.0.0.1:8765";

/// Entry point for `ss-service.exe agent`. Never returns under normal operation;
/// loops forever, reconnecting to the service if the socket drops.
pub fn run() -> ! {
    // Config may not load in every session context; fall back to sane defaults.
    let (fps, monitor_index) = match Config::load() {
        Ok(c) => (c.capture.fps, c.capture.monitor_index),
        Err(_) => (30, 0),
    };

    loop {
        match TcpStream::connect(FRAME_INTAKE_ADDR) {
            Ok(stream) => {
                tracing::info!("Agent connected to service frame intake");
                stream_frames(stream, fps, monitor_index);
                tracing::warn!("Agent lost connection to service - retrying");
            }
            Err(_) => {
                // Service not up yet (or restarting). Wait and retry.
                std::thread::sleep(Duration::from_millis(750));
            }
        }
    }
}

fn stream_frames(mut stream: TcpStream, fps: u32, monitor_index: usize) {
    let _ = stream.set_nodelay(true);
    let mut capture = DesktopCapture::new(monitor_index, fps);

    // Encoder is (re)built whenever the captured dimensions change - which can
    // happen across desktop switches or resolution changes.
    let mut encoder: Option<(JpegEncoder, u32, u32)> = None;
    let frame_sleep = Duration::from_millis((1000 / fps.max(1)).max(5) as u64);

    loop {
        match capture.next_frame() {
            Ok(Some(frame)) => {
                let (w, h) = (frame.width, frame.height);
                let enc = match &mut encoder {
                    Some((e, ew, eh)) if *ew == w && *eh == h => e,
                    _ => {
                        match JpegEncoder::new(w, h, 72) {
                            Ok(e) => {
                                encoder = Some((e, w, h));
                                &mut encoder.as_mut().unwrap().0
                            }
                            Err(e) => {
                                tracing::error!("Encoder init failed: {}", e);
                                std::thread::sleep(frame_sleep);
                                continue;
                            }
                        }
                    }
                };

                match enc.encode(&frame) {
                    Ok(encoded) => {
                        if write_frame(&mut stream, &encoded.data).is_err() {
                            return; // socket dropped - caller reconnects
                        }
                    }
                    Err(e) => tracing::debug!("encode error: {}", e),
                }
            }
            Ok(None) => std::thread::sleep(frame_sleep),
            Err(e) => {
                // Hard capture failure (e.g. no GPU access at all yet). Back off.
                tracing::debug!("capture error: {}", e);
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

fn write_frame(stream: &mut TcpStream, jpeg: &[u8]) -> std::io::Result<()> {
    let len = (jpeg.len() as u32).to_be_bytes();
    stream.write_all(&len)?;
    stream.write_all(jpeg)?;
    Ok(())
}
