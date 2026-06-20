#[macro_use]
extern crate windows_service;

mod agent;
mod win;

use ss_core::config::Config;
use ss_core::consent::ConsentManager;
use ss_core::Result;
use ss_server::HttpServer;
use ss_stream::StreamSession;
use std::ffi::OsString;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::sync::RwLock;

use windows_service::service::*;
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

static SERVICE_NAME: &str = "SSService";

define_windows_service!(ffi_service_main, service_main);

fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service() {
        tracing::error!("Service failed: {}", e);
    }
}

fn run_service() -> Result<()> {
    // Logging is initialized in main() before the service dispatcher starts.
    tracing::info!("{} starting...", SERVICE_NAME);

    // Channel used to signal shutdown from the SCM control handler (which runs on
    // a separate thread) to this thread, which blocks waiting for STOP.
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();

    // The control handler closure must be 'static, so it move-captures its own
    // clone of the sender (`Sender` is `Clone`).
    let tx = shutdown_tx.clone();
    let event_handler = move |control_event: ServiceControl| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                tracing::info!("Service stop requested");
                // Ignore send errors: if the receiver is already gone we are
                // already shutting down.
                let _ = tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
        .map_err(|e| ss_core::Error::Service(format!("Failed to register handler: {}", e)))?;

    let next_status = ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::from_secs(30),
        process_id: None,
    };

    status_handle
        .set_service_status(next_status)
        .map_err(|e| ss_core::Error::Service(format!("Failed to set status: {}", e)))?;

    let config = Config::load()?;
    let consent = ConsentManager::new()?;

    if !consent.is_consent_granted(&config.device.id)? {
        tracing::error!("Consent not granted. Service cannot start.");
        return Err(ss_core::Error::ConsentNotGranted);
    }

    let running_status = ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    };

    status_handle
        .set_service_status(running_status)
        .map_err(|e| ss_core::Error::Service(format!("Failed to set running status: {}", e)))?;

    // `serve_service` blocks forever (HTTP accept loop), so run it on a
    // background thread. This thread stays free to wait for the STOP signal.
    // If the server fails (e.g. HTTP bind error) it returns early; we surface
    // that by signalling shutdown so the service still reports STOPPED instead
    // of hanging in the Running state.
    let server_shutdown_tx = shutdown_tx.clone();
    std::thread::spawn(move || {
        if let Err(e) = serve_service(config) {
            tracing::error!("serve_service exited with error: {}", e);
        } else {
            tracing::warn!("serve_service returned unexpectedly");
        }
        let _ = server_shutdown_tx.send(());
    });

    // Block until a STOP control arrives (or the server thread signals it died).
    let _ = shutdown_rx.recv();
    tracing::info!("Shutdown signalled - stopping service");

    let stop_pending_status = ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StopPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::from_secs(5),
        process_id: None,
    };

    let _ = status_handle.set_service_status(stop_pending_status);

    let stopped_status = ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    };

    let _ = status_handle.set_service_status(stopped_status);

    // Reporting SERVICE_STOPPED with exit code 0 is a CLEAN stop, so the SCM
    // auto-restart failure policy will NOT fire. Exiting the process triggers
    // the kill-on-close Job Object, which terminates the capture agent.
    std::process::exit(0);

    // Unreachable on the normal path; kept so the function type-checks as
    // `Result<()>` consistently with the rest of the service entry points.
    #[allow(unreachable_code)]
    Ok(())
}

fn serve(config: Config) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| ss_core::Error::Service(format!("Failed to create runtime: {}", e)))?;

    rt.block_on(async {
        let session = StreamSession::new(config.clone())?;
        let session = Arc::new(RwLock::new(session));

        session.read().await.start().await?;

        let http_server = HttpServer::new(config.clone(), session.clone());

        tracing::info!(
            "Server starting - open the viewer at http://localhost:{}",
            config.server.http_port
        );

        if let Err(e) = http_server.start().await {
            tracing::error!("HTTP server error: {}", e);
            return Err(e);
        }

        Ok::<(), ss_core::Error>(())
    })
}

/// Service-mode serving: the HTTP server and frame intake run here in Session 0
/// (networking works fine there), while the actual screen capture is performed
/// by an agent process launched into the active desktop session. This is what
/// allows capture of the login / lock / secure desktop.
fn serve_service(config: Config) -> Result<()> {
    // Supervisor runs on its own OS thread using blocking Win32 calls; it keeps a
    // capture agent alive in whichever session currently owns the console.
    if let Ok(exe) = std::env::current_exe() {
        std::thread::spawn(move || supervise_agent(exe));
    } else {
        tracing::error!("Could not determine own exe path - capture agent will not launch");
    }

    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| ss_core::Error::Service(format!("Failed to create runtime: {}", e)))?;

    rt.block_on(async {
        let session = Arc::new(RwLock::new(StreamSession::new_headless(config.clone())));

        // Feed externally-captured frames from the agent into the broadcast.
        let tx = session.read().await.frame_sender();
        tokio::spawn(async move {
            if let Err(e) = run_frame_intake(tx).await {
                tracing::error!("Frame intake stopped: {}", e);
            }
        });

        let http_server = HttpServer::new(config.clone(), session.clone());
        tracing::info!(
            "Server starting - viewer at http://localhost:{}",
            config.server.http_port
        );
        if let Err(e) = http_server.start().await {
            tracing::error!("HTTP server error: {}", e);
            return Err(e);
        }
        Ok::<(), ss_core::Error>(())
    })
}

/// Accept the agent's loopback connection and forward its length-prefixed JPEG
/// frames (`[u32 BE len][jpeg]`) into the viewer broadcast channel.
async fn run_frame_intake(tx: broadcast::Sender<Vec<u8>>) -> Result<()> {
    let listener = TcpListener::bind(agent::FRAME_INTAKE_ADDR)
        .await
        .map_err(|e| ss_core::Error::Server(format!("Failed to bind frame intake: {}", e)))?;
    tracing::info!("Frame intake listening on {}", agent::FRAME_INTAKE_ADDR);

    loop {
        let (mut sock, _) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("Frame intake accept error: {}", e);
                continue;
            }
        };
        tracing::info!("Capture agent connected to frame intake");
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut len_buf = [0u8; 4];
            loop {
                if sock.read_exact(&mut len_buf).await.is_err() {
                    break;
                }
                let len = u32::from_be_bytes(len_buf) as usize;
                // Sanity bound: a JPEG frame over ~50 MB is bogus.
                if len == 0 || len > 50_000_000 {
                    break;
                }
                let mut buf = vec![0u8; len];
                if sock.read_exact(&mut buf).await.is_err() {
                    break;
                }
                let _ = tx.send(buf);
            }
            tracing::warn!("Capture agent disconnected from frame intake");
        });
    }
}

/// Keep a capture agent running in the active console session. Re-launches when
/// the session changes (logon / logoff / fast user switch) or the agent dies.
fn supervise_agent(exe: std::path::PathBuf) {
    // Kill-on-close job: guarantees the agent never outlives this service.
    let job = win::create_kill_on_close_job();
    let mut current: Option<win::AgentProcess> = None;
    loop {
        match win::active_console_session() {
            Some(sid) => {
                let needs_launch = match &current {
                    Some(a) => !a.is_alive() || a.session_id != sid,
                    None => true,
                };
                if needs_launch {
                    if let Some(old) = current.take() {
                        old.terminate();
                    }
                    current = win::launch_agent_in_session(sid, &exe, job);
                }
            }
            None => {
                // No one at the console - stop capturing until someone connects.
                if let Some(old) = current.take() {
                    old.terminate();
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// Run in the foreground in the interactive desktop session. This is the mode
/// that can actually capture the screen (DXGI Desktop Duplication does not work
/// from a Session 0 Windows service).
fn run_console() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("{} starting in console mode...", SERVICE_NAME);

    let config = Config::load()?;
    let consent = ConsentManager::new()?;

    if !consent.is_consent_granted(&config.device.id)? {
        tracing::error!("Consent not granted. Run: ss-cli.exe consent grant");
        return Err(ss_core::Error::ConsentNotGranted);
    }

    serve(config)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Capture-agent mode: launched by the service into the active session.
    if args.iter().any(|a| a == "agent") {
        init_file_logging("ss-agent.log");
        agent::run();
    }

    let console = args
        .iter()
        .any(|a| a == "--console" || a == "-c" || a == "console" || a == "run");

    if console {
        if let Err(e) = run_console() {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Service mode: log to a file since Session 0 has no console to print to.
    init_file_logging("ss-service.log");

    if let Err(_e) = windows_service::service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        eprintln!(
            "Failed to start the Windows service dispatcher.\n\
             To run interactively (recommended for screen capture), use:\n\
             \"%ProgramData%\\ss-service\\ss-service.exe\" --console"
        );
    }
}

/// Logging for the windowless service/agent processes: append to a file under
/// the config dir so behaviour (incl. desktop switches) can be diagnosed.
fn init_file_logging(file_name: &str) {
    use std::io::Write;
    use std::sync::Mutex;

    let path = Config::config_dir().join(file_name);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path);

    #[derive(Clone)]
    struct FileWriter(Arc<Mutex<std::fs::File>>);
    impl Write for FileWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.lock().unwrap().flush()
        }
    }

    match file {
        Ok(f) => {
            let writer = FileWriter(Arc::new(Mutex::new(f)));
            let _ = tracing_subscriber::fmt()
                .with_writer(move || writer.clone())
                .with_ansi(false)
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                )
                .try_init();
        }
        Err(_) => {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                )
                .try_init();
        }
    }
}
