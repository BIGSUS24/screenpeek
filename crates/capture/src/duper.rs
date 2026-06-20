use crate::frame::Frame;
use ss_core::{Error, Result};
use std::time::{Duration, Instant};
use windows::core::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;

/// Result of attempting to grab one frame.
pub enum FrameOutcome {
    /// A new frame was captured.
    Frame(Frame),
    /// No new frame yet (timeout / rate-limited) - just try again.
    None,
    /// The duplication became invalid (desktop switch: lock/unlock, UAC secure
    /// desktop, resolution change, GPU mode switch). The caller must recreate
    /// the duplication - typically after re-attaching to the new input desktop.
    Lost,
}

pub struct DesktopDuplication {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging_texture: ID3D11Texture2D,
    width: u32,
    height: u32,
    last_frame_time: Instant,
    frame_interval: Duration,
}

impl DesktopDuplication {
    pub fn new(monitor_index: usize, fps: u32) -> Result<Self> {
        unsafe {
            let mut device = None;
            let mut context = None;

            let feature_levels: [D3D_FEATURE_LEVEL; 2] = [
                D3D_FEATURE_LEVEL(D3D_FEATURE_LEVEL_11_0.0),
                D3D_FEATURE_LEVEL(D3D_FEATURE_LEVEL_10_1.0),
            ];
            let mut achieved_level = D3D_FEATURE_LEVEL(0);

            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut achieved_level),
                Some(&mut context),
            )
            .map_err(|e| Error::Capture(format!("Failed to create D3D11 device: {}", e)))?;

            let device = device.ok_or_else(|| Error::Capture("Device is null".to_string()))?;
            let context = context.ok_or_else(|| Error::Capture("Context is null".to_string()))?;

            let dxgi_device: IDXGIDevice = device
                .cast()
                .map_err(|e| Error::Capture(format!("Failed to cast to IDXGIDevice: {}", e)))?;

            let adapter: IDXGIAdapter = dxgi_device
                .GetParent()
                .map_err(|e| Error::Capture(format!("Failed to get adapter: {}", e)))?;

            let mut output: Option<IDXGIOutput> = None;
            let mut i = 0u32;
            loop {
                match adapter.EnumOutputs(i) {
                    Ok(out) => {
                        if i as usize == monitor_index {
                            output = Some(out);
                            break;
                        }
                        i += 1;
                        if i > 16 {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }

            let output = output.ok_or_else(|| {
                Error::Capture(format!("Monitor index {} not found", monitor_index))
            })?;

            let output1: IDXGIOutput1 = output
                .cast()
                .map_err(|e| Error::Capture(format!("Failed to cast to IDXGIOutput1: {}", e)))?;

            let duplication = output1
                .DuplicateOutput(&device)
                .map_err(|e| Error::Capture(format!("Failed to duplicate output: {}", e)))?;

            let desc = output
                .GetDesc()
                .map_err(|e| Error::Capture(format!("Failed to get output desc: {}", e)))?;

            let width = (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left) as u32;
            let height = (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top) as u32;

            let tex_desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };

            let mut staging_texture = None;
            device
                .CreateTexture2D(&tex_desc, None, Some(&mut staging_texture))
                .map_err(|e| Error::Capture(format!("Failed to create staging texture: {}", e)))?;

            let staging_texture = staging_texture
                .ok_or_else(|| Error::Capture("Staging texture is null".to_string()))?;

            let frame_interval = Duration::from_micros(1_000_000 / fps as u64);

            Ok(Self {
                device,
                context,
                duplication,
                staging_texture,
                width,
                height,
                last_frame_time: Instant::now(),
                frame_interval,
            })
        }
    }

    pub fn capture_frame(&mut self) -> Result<Option<Frame>> {
        let now = Instant::now();
        if now.duration_since(self.last_frame_time) < self.frame_interval {
            return Ok(None);
        }

        unsafe {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;

            let result = self
                .duplication
                .AcquireNextFrame(16, &mut frame_info, &mut resource);

            match result {
                Ok(()) => {
                    let resource = resource
                        .ok_or_else(|| Error::Capture("Frame resource is null".to_string()))?;

                    let texture: ID3D11Texture2D = resource
                        .cast()
                        .map_err(|e| Error::Capture(format!("Failed to cast texture: {}", e)))?;

                    self.context
                        .CopyResource(&self.staging_texture, &texture);

                    self.duplication
                        .ReleaseFrame()
                        .map_err(|e| Error::Capture(format!("Failed to release frame: {}", e)))?;

                    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                    self.context.Map(
                        &self.staging_texture,
                        0,
                        D3D11_MAP_READ,
                        0,
                        Some(&mut mapped),
                    )
                    .map_err(|e| Error::Capture(format!("Map failed: {}", e)))?;

                    let data_size = mapped.RowPitch as usize * self.height as usize;
                    let data = std::slice::from_raw_parts(mapped.pData as *const u8, data_size)
                        .to_vec();

                    self.context.Unmap(&self.staging_texture, 0);

                    self.last_frame_time = now;

                    let timestamp_us = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_micros() as u64;

                    Ok(Some(Frame::new(
                        self.width,
                        self.height,
                        mapped.RowPitch as u32,
                        data,
                        timestamp_us,
                    )))
                }
                Err(e) => {
                    if e.code() == DXGI_ERROR_WAIT_TIMEOUT {
                        Ok(None)
                    } else {
                        Err(Error::Capture(format!("AcquireNextFrame failed: {}", e)))
                    }
                }
            }
        }
    }

    /// Like `capture_frame`, but reports desktop-switch loss as a recoverable
    /// `FrameOutcome::Lost` instead of a hard error. Used by `DesktopCapture`.
    pub fn capture_outcome(&mut self) -> FrameOutcome {
        let now = Instant::now();
        if now.duration_since(self.last_frame_time) < self.frame_interval {
            return FrameOutcome::None;
        }

        unsafe {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;

            let result = self
                .duplication
                .AcquireNextFrame(16, &mut frame_info, &mut resource);

            match result {
                Ok(()) => {
                    let texture: ID3D11Texture2D = match resource.and_then(|r| r.cast().ok()) {
                        Some(t) => t,
                        None => {
                            let _ = self.duplication.ReleaseFrame();
                            return FrameOutcome::None;
                        }
                    };

                    self.context.CopyResource(&self.staging_texture, &texture);
                    let _ = self.duplication.ReleaseFrame();

                    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                    if self
                        .context
                        .Map(&self.staging_texture, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                        .is_err()
                    {
                        return FrameOutcome::None;
                    }

                    let data_size = mapped.RowPitch as usize * self.height as usize;
                    let data =
                        std::slice::from_raw_parts(mapped.pData as *const u8, data_size).to_vec();
                    self.context.Unmap(&self.staging_texture, 0);

                    self.last_frame_time = now;

                    let timestamp_us = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_micros() as u64;

                    FrameOutcome::Frame(Frame::new(
                        self.width,
                        self.height,
                        mapped.RowPitch as u32,
                        data,
                        timestamp_us,
                    ))
                }
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => FrameOutcome::None,
                // Desktop switched (lock/unlock, UAC secure desktop, resolution
                // change) or access was revoked - the caller must recreate.
                Err(_) => FrameOutcome::Lost,
            }
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
