#[derive(Debug, Clone)]
pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub is_keyframe: bool,
    pub timestamp_us: u64,
    pub width: u32,
    pub height: u32,
}

impl EncodedFrame {
    pub fn new(
        data: Vec<u8>,
        is_keyframe: bool,
        timestamp_us: u64,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            data,
            is_keyframe,
            timestamp_us,
            width,
            height,
        }
    }
}
