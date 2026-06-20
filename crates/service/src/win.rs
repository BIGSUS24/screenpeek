//! Windows session bridging.
//!
//! The service runs as LocalSystem in Session 0, which cannot capture the
//! interactive desktop. To capture it (including the secure desktop) we launch
//! the capture agent *into the active console session* as SYSTEM:
//!
//! 1. `WTSGetActiveConsoleSessionId` - find the session showing on the monitor.
//! 2. Duplicate our own SYSTEM token, then set its session id to that session.
//! 3. `CreateProcessAsUserW` with that token - the agent now runs as SYSTEM in
//!    the interactive session and can attach to the input/secure desktop.
//!
//! We re-launch whenever the active session changes (logon / logoff / fast user
//! switch) or the agent dies, so capture follows the user.

#![cfg(windows)]

use std::path::Path;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    DuplicateTokenEx, SetTokenInformation, SecurityIdentification, TokenPrimary, TokenSessionId,
    TOKEN_ACCESS_MASK, TOKEN_ADJUST_DEFAULT, TOKEN_ADJUST_SESSIONID, TOKEN_ASSIGN_PRIMARY,
    TOKEN_DUPLICATE, TOKEN_QUERY,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
    JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::RemoteDesktop::{WTSGetActiveConsoleSessionId, WTSQueryUserToken};
use windows::Win32::System::Threading::{
    CreateProcessAsUserW, GetCurrentProcess, GetExitCodeProcess, OpenProcessToken,
    TerminateProcess, CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, PROCESS_INFORMATION,
    STARTUPINFOW,
};

const STILL_ACTIVE: u32 = 259;
const INVALID_SESSION: u32 = 0xFFFF_FFFF;

/// The session currently attached to the physical console (keyboard/monitor),
/// or `None` if no one is connected (e.g. between logoff and the next logon).
pub fn active_console_session() -> Option<u32> {
    let id = unsafe { WTSGetActiveConsoleSessionId() };
    if id == INVALID_SESSION {
        None
    } else {
        Some(id)
    }
}

/// A running capture-agent process we supervise.
pub struct AgentProcess {
    process: HANDLE,
    pub session_id: u32,
}

impl AgentProcess {
    /// Whether the agent is still running.
    pub fn is_alive(&self) -> bool {
        unsafe {
            let mut code: u32 = 0;
            if GetExitCodeProcess(self.process, &mut code).is_ok() {
                code == STILL_ACTIVE
            } else {
                false
            }
        }
    }

    /// Force-kill the agent (used when the active session changes).
    pub fn terminate(&self) {
        unsafe {
            let _ = TerminateProcess(self.process, 1);
        }
    }
}

impl Drop for AgentProcess {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.process);
        }
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Create a job object that kills all assigned processes when its last handle
/// closes. We assign the agent to it so the agent can never outlive the service
/// (no orphaned capture process). The returned handle must be kept alive for the
/// service's lifetime.
pub fn create_kill_on_close_job() -> Option<HANDLE> {
    unsafe {
        let job = CreateJobObjectW(None, PCWSTR::null()).ok()?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .is_err()
        {
            tracing::warn!("SetInformationJobObject failed - agent may outlive service");
            let _ = CloseHandle(job);
            return None;
        }
        Some(job)
    }
}

/// Acquire the primary token to launch the capture agent with for `session_id`.
///
/// We prefer the *interactive user's* token, because DXGI Desktop Duplication run
/// under the SYSTEM account returns an all-black image for the normal desktop. A
/// user-token process can capture the real desktop. The service runs as
/// LocalSystem (holds SE_TCB), so `WTSQueryUserToken` is permitted.
///
/// Returns a primary token ready for `CreateProcessAsUserW`. The caller owns the
/// returned handle and must `CloseHandle` it. Returns `None` only on hard failure
/// (both the user-token path and the SYSTEM fallback failed).
unsafe fn acquire_agent_token(session_id: u32) -> Option<HANDLE> {
    // 1. Try the logged-in user's token for this session first.
    let mut user_token = HANDLE::default();
    if WTSQueryUserToken(session_id, &mut user_token).is_ok() {
        // Duplicate into a primary token we can hand to a new process. It's
        // already in the correct session, so no SetTokenInformation needed.
        let mut dup_token = HANDLE::default();
        let dup_res = DuplicateTokenEx(
            user_token,
            TOKEN_ACCESS_MASK(0x0200_0000), // MAXIMUM_ALLOWED
            None,
            SecurityIdentification,
            TokenPrimary,
            &mut dup_token,
        );
        let _ = CloseHandle(user_token);
        if dup_res.is_ok() {
            tracing::info!("Launching agent as logged-in user (session {})", session_id);
            return Some(dup_token);
        }
        tracing::warn!(
            "DuplicateTokenEx on user token failed; falling back to SYSTEM token"
        );
    } else {
        tracing::debug!(
            "WTSQueryUserToken(session {}) failed (no interactive user / secure desktop); \
             falling back to SYSTEM token",
            session_id
        );
    }

    // 2. Fallback: duplicate our own SYSTEM token and retarget it to the session.
    //    (Capture will be black for the normal desktop, but this preserves prior
    //    behaviour at the lock/secure desktop where no user token is available.)
    let mut our_token = HANDLE::default();
    if OpenProcessToken(
        GetCurrentProcess(),
        TOKEN_DUPLICATE
            | TOKEN_QUERY
            | TOKEN_ASSIGN_PRIMARY
            | TOKEN_ADJUST_DEFAULT
            | TOKEN_ADJUST_SESSIONID,
        &mut our_token,
    )
    .is_err()
    {
        tracing::error!("OpenProcessToken failed");
        return None;
    }

    let mut dup_token = HANDLE::default();
    let dup_res = DuplicateTokenEx(
        our_token,
        TOKEN_ACCESS_MASK(0x0200_0000), // MAXIMUM_ALLOWED
        None,
        SecurityIdentification,
        TokenPrimary,
        &mut dup_token,
    );
    let _ = CloseHandle(our_token);
    if dup_res.is_err() {
        tracing::error!("DuplicateTokenEx failed");
        return None;
    }

    // Retarget the duplicated SYSTEM token to the active console session.
    let sid = session_id;
    if SetTokenInformation(
        dup_token,
        TokenSessionId,
        &sid as *const u32 as *const core::ffi::c_void,
        std::mem::size_of::<u32>() as u32,
    )
    .is_err()
    {
        tracing::error!("SetTokenInformation(session {}) failed", session_id);
        let _ = CloseHandle(dup_token);
        return None;
    }

    tracing::info!(
        "Launching agent as SYSTEM (no interactive user, session {})",
        session_id
    );
    Some(dup_token)
}

/// Launch the capture agent inside `session_id`. Prefers the logged-in user's
/// token (required for DXGI to capture the real desktop) and falls back to a
/// SYSTEM token when no interactive user is present. If `job` is provided the
/// agent is assigned to it so it dies with the service. Returns the running
/// process handle on success.
pub fn launch_agent_in_session(
    session_id: u32,
    exe_path: &Path,
    job: Option<HANDLE>,
) -> Option<AgentProcess> {
    unsafe {
        // 1-3. Acquire the token to launch with (user-token preferred, SYSTEM fallback).
        let dup_token = acquire_agent_token(session_id)?;

        // 4. Build command line: "<exe>" agent  (must be a writable buffer).
        let app = to_wide(&exe_path.to_string_lossy());
        let mut cmd = to_wide(&format!("\"{}\" agent", exe_path.to_string_lossy()));
        // Interactive window station + default desktop; the agent re-attaches to
        // whatever desktop currently has input (incl. the secure desktop).
        let mut desktop = to_wide("winsta0\\default");

        let si = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            lpDesktop: PWSTR(desktop.as_mut_ptr()),
            ..Default::default()
        };
        let mut pi = PROCESS_INFORMATION::default();

        let created = CreateProcessAsUserW(
            dup_token,
            PCWSTR(app.as_ptr()),
            PWSTR(cmd.as_mut_ptr()),
            None,
            None,
            false,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
            None,
            PCWSTR::null(),
            &si,
            &mut pi,
        );
        let _ = CloseHandle(dup_token);

        match created {
            Ok(()) => {
                let _ = CloseHandle(pi.hThread);
                if let Some(job) = job {
                    if AssignProcessToJobObject(job, pi.hProcess).is_err() {
                        tracing::warn!("AssignProcessToJobObject failed");
                    }
                }
                tracing::info!(
                    "Launched capture agent (pid {}) in session {}",
                    pi.dwProcessId,
                    session_id
                );
                Some(AgentProcess {
                    process: pi.hProcess,
                    session_id,
                })
            }
            Err(e) => {
                tracing::error!("CreateProcessAsUserW failed: {}", e);
                None
            }
        }
    }
}
