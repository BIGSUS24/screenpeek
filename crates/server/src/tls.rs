use ss_core::config::Config;
use ss_core::{Error, Result};
use std::path::PathBuf;

pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

impl TlsConfig {
    pub fn from_config(config: &Config) -> Result<Self> {
        let tls_dir = Config::tls_dir();

        let cert_path = config
            .server
            .tls_cert_path
            .clone()
            .unwrap_or_else(|| tls_dir.join("cert.pem"));

        let key_path = config
            .server
            .tls_key_path
            .clone()
            .unwrap_or_else(|| tls_dir.join("key.pem"));

        if !cert_path.exists() || !key_path.exists() {
            return Err(Error::Config(
                "TLS certificate or key not found. Generate them first.".to_string(),
            ));
        }

        Ok(Self { cert_path, key_path })
    }

    pub fn generate_self_signed() -> Result<Self> {
        let tls_dir = Config::tls_dir();
        std::fs::create_dir_all(&tls_dir)?;

        let cert_path = tls_dir.join("cert.pem");
        let key_path = tls_dir.join("key.pem");

        let cert = generate_certificate()?;
        std::fs::write(&cert_path, &cert.cert_pem)?;
        std::fs::write(&key_path, &cert.private_key_pem)?;

        Ok(Self { cert_path, key_path })
    }
}

struct Certificate {
    cert_pem: Vec<u8>,
    private_key_pem: Vec<u8>,
}

fn generate_certificate() -> Result<Certificate> {
    use rcgen::{CertificateParams, KeyPair};

    let key_pair = KeyPair::generate().map_err(|e| Error::Config(format!("Key generation failed: {}", e)))?;

    let mut params = CertificateParams::new(vec!["localhost".to_string()])
        .map_err(|e| Error::Config(format!("Certificate params failed: {}", e)))?;

    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "SS Service");

    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| Error::Config(format!("Certificate signing failed: {}", e)))?;

    Ok(Certificate {
        cert_pem: cert.pem().into_bytes(),
        private_key_pem: key_pair.serialize_pem().into_bytes(),
    })
}
