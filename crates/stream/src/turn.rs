use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnServer {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

impl TurnServer {
    pub fn public_stun() -> Self {
        Self {
            urls: vec![
                "stun:stun.l.google.com:19302".to_string(),
                "stun:stun1.l.google.com:19302".to_string(),
            ],
            username: None,
            credential: None,
        }
    }

    pub fn custom(urls: Vec<String>, username: String, credential: String) -> Self {
        Self {
            urls,
            username: Some(username),
            credential: Some(credential),
        }
    }

    pub fn to_ice_server(&self) -> serde_json::Value {
        let mut server = serde_json::json!({
            "urls": self.urls,
        });

        if let Some(username) = &self.username {
            server["username"] = serde_json::Value::String(username.clone());
        }
        if let Some(credential) = &self.credential {
            server["credential"] = serde_json::Value::String(credential.clone());
        }

        server
    }
}
