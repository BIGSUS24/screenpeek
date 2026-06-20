use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdpOffer {
    pub sdp: String,
    pub sdp_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdpAnswer {
    pub sdp: String,
    pub sdp_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_mid: String,
    pub sdp_m_line_index: u32,
}

pub struct PeerConnection {
    pub id: String,
    pub viewer_id: String,
    pub state: PeerState,
    pub offer: Option<SdpOffer>,
    pub answer: Option<SdpAnswer>,
    pub ice_candidates: Vec<IceCandidate>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PeerState {
    New,
    Connected,
    Disconnected,
    Failed,
}

impl PeerConnection {
    pub fn new(id: String, viewer_id: String) -> Self {
        Self {
            id,
            viewer_id,
            state: PeerState::New,
            offer: None,
            answer: None,
            ice_candidates: Vec::new(),
        }
    }

    pub fn set_offer(&mut self, offer: SdpOffer) {
        self.offer = Some(offer);
        self.state = PeerState::Connected;
    }

    pub fn set_answer(&mut self, answer: SdpAnswer) {
        self.answer = Some(answer);
    }

    pub fn add_ice_candidate(&mut self, candidate: IceCandidate) {
        self.ice_candidates.push(candidate);
    }
}
