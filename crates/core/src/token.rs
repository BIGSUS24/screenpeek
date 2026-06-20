use crate::config::Config;
use crate::crypto;
use crate::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Token {
    pub viewer_id: String,
    pub device_id: String,
    pub issued_at: String,
    pub expires_at: String,
    pub nonce: String,
}

pub struct TokenManager {
    secret: Vec<u8>,
    expiry_secs: u64,
}

impl TokenManager {
    pub fn new(config: &Config) -> Result<Self> {
        let secret = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &config.auth.token_secret,
        )
        .map_err(|e| Error::Token(e.to_string()))?;

        Ok(Self {
            secret,
            expiry_secs: config.auth.token_expiry_secs,
        })
    }

    pub fn create_token(&self, viewer_id: &str, device_id: &str) -> Result<String> {
        let now = Utc::now();
        let expires = now + chrono::Duration::seconds(self.expiry_secs as i64);

        let nonce = {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let bytes: Vec<u8> = (0..16).map(|_| rng.gen()).collect();
            crypto::base64_encode(&bytes)
        };

        let token = Token {
            viewer_id: viewer_id.to_string(),
            device_id: device_id.to_string(),
            issued_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
            nonce,
        };

        let payload = serde_json::to_vec(&token)?;
        let encoded_payload = crypto::base64_encode(&payload);
        let signature = crypto::hmac_sign(&self.secret, encoded_payload.as_bytes());
        let encoded_signature = crypto::base64_encode(&signature);

        Ok(format!("{}.{}", encoded_payload, encoded_signature))
    }

    pub fn verify_token(&self, token_str: &str) -> Result<Token> {
        let parts: Vec<&str> = token_str.split('.').collect();
        if parts.len() != 2 {
            return Err(Error::Auth("Invalid token format".to_string()));
        }

        let (encoded_payload, encoded_signature) = (parts[0], parts[1]);

        let signature = crypto::base64_decode(encoded_signature)
            .map_err(|e| Error::Auth(e.to_string()))?;
        if !crypto::hmac_verify(&self.secret, encoded_payload.as_bytes(), &signature) {
            return Err(Error::Auth("Invalid token signature".to_string()));
        }

        let payload = crypto::base64_decode(encoded_payload)
            .map_err(|e| Error::Auth(e.to_string()))?;
        let token: Token = serde_json::from_slice(&payload)?;

        let now = Utc::now();
        let expires: DateTime<Utc> = DateTime::parse_from_rfc3339(&token.expires_at)
            .map_err(|e| Error::Auth(e.to_string()))?
            .with_timezone(&Utc);

        if now > expires {
            return Err(Error::Auth("Token expired".to_string()));
        }

        Ok(token)
    }
}
