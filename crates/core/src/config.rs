use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::device::DeviceId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub device: DeviceConfig,
    pub server: ServerConfig,
    pub capture: CaptureConfig,
    pub encoder: EncoderConfig,
    pub auth: AuthConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub id: DeviceId,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub http_port: u16,
    pub https_port: u16,
    pub tls_cert_path: Option<PathBuf>,
    pub tls_key_path: Option<PathBuf>,
    pub max_viewers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    pub fps: u32,
    pub monitor_index: usize,
    pub capture_cursor: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderConfig {
    pub codec: String,
    pub bitrate: u32,
    pub keyframe_interval: u32,
    pub preset: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub token_secret: String,
    pub token_expiry_secs: u64,
    pub allowed_viewers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device: DeviceConfig {
                id: DeviceId::new(),
                name: hostname::get()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "unknown".to_string()),
                created_at: chrono::Utc::now().to_rfc3339(),
            },
            server: ServerConfig {
                http_port: 8080,
                https_port: 8443,
                tls_cert_path: None,
                tls_key_path: None,
                max_viewers: 3,
            },
            capture: CaptureConfig {
                fps: 30,
                monitor_index: 0,
                capture_cursor: true,
            },
            encoder: EncoderConfig {
                codec: "h264".to_string(),
                bitrate: 2_000_000,
                keyframe_interval: 2,
                preset: "balanced".to_string(),
            },
            auth: AuthConfig {
                token_secret: generate_secret(),
                token_expiry_secs: 3600,
                allowed_viewers: Vec::new(),
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file: None,
            },
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        let base = std::env::var("ProgramData")
            .unwrap_or_else(|_| "C:\\ProgramData".to_string());
        PathBuf::from(base).join("ss-service")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn consent_path() -> PathBuf {
        Self::config_dir().join("consent.bin")
    }

    pub fn tls_dir() -> PathBuf {
        Self::config_dir().join("tls")
    }

    pub fn load() -> crate::Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            return Err(crate::Error::Config(
                "Configuration file not found. Run 'ss-cli setup' first.".to_string(),
            ));
        }
        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> crate::Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;
        let content = toml::to_string_pretty(self)?;
        std::fs::write(Self::config_path(), content)?;
        Ok(())
    }
}

fn generate_secret() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes)
}
