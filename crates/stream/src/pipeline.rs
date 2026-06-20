use ss_capture::{DesktopDuplication};
use ss_encoder::JpegEncoder;
use ss_core::Result;

pub struct Pipeline {
    capture: DesktopDuplication,
    encoder: JpegEncoder,
}

impl Pipeline {
    pub fn new(fps: u32, monitor_index: usize, _bitrate: u32, _keyframe_interval: u32) -> Result<Self> {
        let capture = DesktopDuplication::new(monitor_index, fps)?;
        let (width, height) = capture.dimensions();
        // JPEG quality ~72 is a good size/quality balance for screen content.
        let encoder = JpegEncoder::new(width, height, 72)?;

        Ok(Self { capture, encoder })
    }

    pub fn capture_and_encode(&mut self) -> Result<Option<ss_encoder::EncodedFrame>> {
        match self.capture.capture_frame()? {
            Some(frame) => {
                let encoded = self.encoder.encode(&frame)?;
                Ok(Some(encoded))
            }
            None => Ok(None),
        }
    }

    /// Adjust JPEG quality (1-100). Higher = better image, larger frames.
    pub fn set_quality(&mut self, quality: u8) {
        self.encoder.set_quality(quality);
    }
}
