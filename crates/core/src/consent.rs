use crate::config::Config;
use crate::device::DeviceId;
use crate::{Error, Result};
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize)]
struct ConsentData {
    device_id: DeviceId,
    granted_at: String,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

pub struct ConsentManager {
    encryption_key: [u8; 32],
}

impl ConsentManager {
    pub fn new() -> Result<Self> {
        let key_path = Config::config_dir().join("consent.key");
        let encryption_key = if key_path.exists() {
            let key_bytes = std::fs::read(&key_path)?;
            if key_bytes.len() != 32 {
                return Err(Error::ConsentNotGranted);
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&key_bytes);
            key
        } else {
            let mut key = [0u8; 32];
            OsRng.fill_bytes(&mut key);
            std::fs::create_dir_all(Config::config_dir())?;
            std::fs::write(&key_path, &key)?;
            key
        };

        Ok(Self { encryption_key })
    }

    pub fn is_consent_granted(&self, device_id: &DeviceId) -> Result<bool> {
        let path = Config::consent_path();
        if !path.exists() {
            return Ok(false);
        }

        let data = std::fs::read(&path)?;
        let consent: ConsentData = serde_json::from_slice(&data)?;

        if &consent.device_id != device_id {
            return Ok(false);
        }

        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
            .map_err(|e| Error::Encryption(e.to_string()))?;
        let nonce = Nonce::from_slice(&consent.nonce);

        let plaintext = cipher
            .decrypt(nonce, consent.ciphertext.as_ref())
            .map_err(|e| Error::Encryption(e.to_string()))?;

        let granted: bool = serde_json::from_slice(&plaintext)?;
        Ok(granted)
    }

    pub fn grant_consent(&self, device_id: &DeviceId) -> Result<()> {
        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
            .map_err(|e| Error::Encryption(e.to_string()))?;

        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = serde_json::to_vec(&true)?;
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|e| Error::Encryption(e.to_string()))?;

        let consent = ConsentData {
            device_id: device_id.clone(),
            granted_at: chrono::Utc::now().to_rfc3339(),
            nonce: nonce_bytes.to_vec(),
            ciphertext,
        };

        let data = serde_json::to_vec(&consent)?;
        std::fs::create_dir_all(Config::config_dir())?;
        std::fs::write(Config::consent_path(), data)?;

        Ok(())
    }

    pub fn revoke_consent(&self) -> Result<()> {
        let path = Config::consent_path();
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}
