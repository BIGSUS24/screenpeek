use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: Arc<Vec<u8>>,
    pub timestamp_us: u64,
    pub is_new: bool,
}

impl Frame {
    pub fn new(width: u32, height: u32, stride: u32, data: Vec<u8>, timestamp_us: u64) -> Self {
        Self {
            width,
            height,
            stride,
            data: Arc::new(data),
            timestamp_us,
            is_new: true,
        }
    }

    pub fn size_bytes(&self) -> usize {
        (self.stride as usize) * (self.height as usize)
    }
}
