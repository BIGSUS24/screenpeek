//! Desktop-following screen capture.
//!
//! Wraps [`DesktopDuplication`] with two behaviours needed to capture the *whole*
//! Windows experience - including the login screen, lock screen and UAC prompts:
//!
//! 1. **Input-desktop attachment.** The interactive session has several desktops
//!    (`Default` for the normal session, `Winlogon` for the secure desktop shown
//!    while locked / at login / during UAC). DXGI duplication only sees the desktop
//!    the calling thread is attached to. Before each (re)create we attach this
//!    thread to whatever desktop currently has input focus via `OpenInputDesktop` +
//!    `SetThreadDesktop`. Reaching the `Winlogon` desktop requires the process to
//!    run as **SYSTEM** - which is why the capture agent is launched by the service.
//!
//! 2. **Loss recovery.** Switching between `Default` and `Winlogon` (every
//!    lock/unlock/UAC) invalidates the duplication (`DXGI_ERROR_ACCESS_LOST`). On
//!    loss we drop the duplication and rebuild it attached to the new input desktop.

use crate::duper::{DesktopDuplication, FrameOutcome};
use crate::frame::Frame;
use ss_core::Result;

#[cfg(windows)]
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, OpenInputDesktop, SetThreadDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS,
};
#[cfg(windows)]
use windows::Win32::System::StationsAndDesktops::HDESK;

pub struct DesktopCapture {
    monitor_index: usize,
    fps: u32,
    dup: Option<DesktopDuplication>,
    /// The desktop handle this thread is currently attached to. Kept so we can
    /// close the *previous* one when switching (the thread has left it by then)
    /// and the last one on drop - otherwise we leak one HDESK per lock/unlock.
    #[cfg(windows)]
    desktop: Option<HDESK>,
}

impl DesktopCapture {
    pub fn new(monitor_index: usize, fps: u32) -> Self {
        Self {
            monitor_index,
            fps,
            dup: None,
            #[cfg(windows)]
            desktop: None,
        }
    }

    /// Attach the current thread to the desktop that currently owns input. This
    /// is what lets a SYSTEM process follow the secure (Winlogon) desktop. Best
    /// effort: if it fails we still try to capture the default desktop.
    #[cfg(windows)]
    fn attach_input_desktop(&mut self) {
        unsafe {
            // GENERIC_ALL = 0x10000000. We ask for full access so SetThreadDesktop
            // and subsequent duplication succeed on the secure desktop.
            match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_ACCESS_FLAGS(0x10000000))
            {
                Ok(hdesk) => {
                    if let Err(e) = SetThreadDesktop(hdesk) {
                        tracing::debug!("SetThreadDesktop failed: {}", e);
                        // The switch did not take effect; close to avoid a leak.
                        let _ = CloseDesktop(hdesk);
                        return;
                    }
                    // The thread has now left the previous desktop, so closing it
                    // is safe. Closing the *current* desktop's handle while a thread
                    // still uses it would be wrong, hence we only close the old one.
                    if let Some(old) = self.desktop.take() {
                        let _ = CloseDesktop(old);
                    }
                    self.desktop = Some(hdesk);
                }
                Err(e) => tracing::debug!("OpenInputDesktop failed (need SYSTEM?): {}", e),
            }
        }
    }

    #[cfg(not(windows))]
    fn attach_input_desktop(&mut self) {}

    fn ensure(&mut self) -> Result<()> {
        if self.dup.is_none() {
            self.attach_input_desktop();
            self.dup = Some(DesktopDuplication::new(self.monitor_index, self.fps)?);
        }
        Ok(())
    }

    /// Capture one frame, transparently recovering from desktop switches.
    /// Returns `Ok(None)` when there is simply no new frame yet, or when a switch
    /// was detected (the next call rebuilds on the new desktop).
    pub fn next_frame(&mut self) -> Result<Option<Frame>> {
        self.ensure()?;
        match self.dup.as_mut().expect("ensured").capture_outcome() {
            FrameOutcome::Frame(f) => Ok(Some(f)),
            FrameOutcome::None => Ok(None),
            FrameOutcome::Lost => {
                tracing::info!("Capture lost (desktop switch) - rebuilding");
                self.dup = None;
                Ok(None)
            }
        }
    }

    pub fn dimensions(&self) -> Option<(u32, u32)> {
        self.dup.as_ref().map(|d| d.dimensions())
    }
}

impl Drop for DesktopCapture {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Some(d) = self.desktop.take() {
            unsafe {
                let _ = CloseDesktop(d);
            }
        }
    }
}
