use crate::frame::EncodedFrame;
use jpeg_encoder::{ColorType, Encoder};
use ss_capture::Frame;
use ss_core::{Error, Result};

/// Real, pure-Rust encoder: converts a captured BGRA desktop frame into a
/// baseline JPEG. Used for MJPEG-over-HTTP streaming. No native build deps.
pub struct JpegEncoder {
    width: u32,
    height: u32,
    quality: u8,
}

impl JpegEncoder {
    pub fn new(width: u32, height: u32, quality: u8) -> Result<Self> {
        Ok(Self {
            width,
            height,
            quality: quality.clamp(1, 100),
        })
    }

    pub fn encode(&mut self, frame: &Frame) -> Result<EncodedFrame> {
        const BPP: usize = 4; // BGRA
        let width = self.width as usize;
        let height = self.height as usize;
        let tight_stride = width * BPP;
        let src_stride = frame.stride as usize;
        let src = frame.data.as_ref();

        // DXGI rows are padded to RowPitch; JPEG needs tightly-packed rows.
        let packed: Vec<u8> = if src_stride == tight_stride && src.len() >= tight_stride * height {
            src[..tight_stride * height].to_vec()
        } else {
            let mut out = Vec::with_capacity(tight_stride * height);
            for row in 0..height {
                let start = row * src_stride;
                let end = start + tight_stride;
                if end <= src.len() {
                    out.extend_from_slice(&src[start..end]);
                } else {
                    // Short buffer: pad with black so dimensions stay valid.
                    out.resize(tight_stride * (row + 1), 0);
                }
            }
            out
        };

        let mut buf = Vec::new();
        let encoder = Encoder::new(&mut buf, self.quality);
        encoder
            .encode(
                &packed,
                self.width as u16,
                self.height as u16,
                ColorType::Bgra,
            )
            .map_err(|e| Error::Encoder(format!("JPEG encode failed: {}", e)))?;

        Ok(EncodedFrame::new(
            buf,
            true,
            frame.timestamp_us,
            self.width,
            self.height,
        ))
    }

    pub fn set_quality(&mut self, quality: u8) {
        self.quality = quality.clamp(1, 100);
    }
}
