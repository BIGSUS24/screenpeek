use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Consent not granted")]
    ConsentNotGranted,

    #[error("Device not registered")]
    DeviceNotRegistered,

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Token error: {0}")]
    Token(String),

    #[error("Capture error: {0}")]
    Capture(String),

    #[error("Encoder error: {0}")]
    Encoder(String),

    #[error("Stream error: {0}")]
    Stream(String),

    #[error("Server error: {0}")]
    Server(String),

    #[error("Service error: {0}")]
    Service(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("TOML error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("Encryption error: {0}")]
    Encryption(String),
}

pub type Result<T> = std::result::Result<T, Error>;
