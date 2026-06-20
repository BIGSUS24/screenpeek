use crate::frame::EncodedFrame;
use ss_capture::Frame;
use ss_core::Result;

pub struct HardwareEncoder {
    width: u32,
    height: u32,
    bitrate: u32,
    frame_count: u64,
    keyframe_interval: u32,
}

impl HardwareEncoder {
    pub fn new(width: u32, height: u32, bitrate: u32, keyframe_interval: u32) -> Result<Self> {
        Ok(Self {
            width,
            height,
            bitrate,
            frame_count: 0,
            keyframe_interval,
        })
    }

    pub fn encode(&mut self, frame: &Frame) -> Result<EncodedFrame> {
        self.frame_count += 1;

        let is_keyframe = self.frame_count == 1 || self.frame_count % self.keyframe_interval as u64 == 0;

        let encoded_data = self.encode_frame(frame, is_keyframe)?;

        Ok(EncodedFrame::new(
            encoded_data,
            is_keyframe,
            frame.timestamp_us,
            self.width,
            self.height,
        ))
    }

    fn encode_frame(&self, frame: &Frame, is_keyframe: bool) -> Result<Vec<u8>> {
        let mut encoded = Vec::with_capacity(frame.data.len() / 4);

        if is_keyframe {
            encoded.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
            encoded.push(0x67);
            encoded.push(0x42);
            encoded.push(0x00);
            encoded.push(0x1E);
            encoded.extend_from_slice(&self.width.to_be_bytes());
            encoded.extend_from_slice(&self.height.to_be_bytes());
        }

        encoded.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        encoded.push(0x41);

        let mb_width = (self.width + 15) / 16;
        let mb_height = (self.height + 15) / 16;
        encoded.extend_from_slice(&mb_width.to_be_bytes());
        encoded.extend_from_slice(&mb_height.to_be_bytes());

        let data_size = frame.data.len().min(1024);
        encoded.extend_from_slice(&frame.data[..data_size]);

        Ok(encoded)
    }

    pub fn set_bitrate(&mut self, bitrate: u32) {
        self.bitrate = bitrate;
    }
}
