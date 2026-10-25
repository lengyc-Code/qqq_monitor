//! One instance of each executable. The running instance holds a named mutex derived from
//! the executable's path, and its hidden tray window carries the same name, so a second
//! start of that executable finds the instance and asks it for its window. Copies of the
//! program at other paths, such as a development build, run alongside it.

use crate::tray;
use std::{
    sync::OnceLock,
    thread,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, E_ACCESSDENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, LPARAM,
            WAIT_ABANDONED, WAIT_OBJECT_0, WPARAM,
        },
        System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
        UI::WindowsAndMessaging::{
            AllowSetForegroundWindow, FindWindowW, GetWindowThreadProcessId, PostMessageW,
        },
    },
    core::HSTRING,
};

/// Argument of a restart: the new instance waits for the running one to exit, then starts.
pub const ARGUMENT: &str = "--replace";
/// Longest wait of a restart for the instance it replaces.
const REPLACE_WAIT: Duration = Duration::from_secs(15);
/// Longest wait for the running instance's tray window, which appears with its window.
const FIND_WAIT: Duration = Duration::from_secs(3);
/// Interval between the attempts of both waits.
const POLL: Duration = Duration::from_millis(100);

/// Name of this executable's instance: the program name and a hash of the executable's
/// full path, which Windows compares without regard to case.
pub(crate) fn name() -> &'static str {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| {
        let path = std::env::current_exe()
            .and_then(std::fs::canonicalize)
            .unwrap_or_default();
        name_of(&path.to_string_lossy())
    })
}

fn name_of(path: &str) -> String {
    // 64-bit FNV-1a, which stays the same across builds and toolchains.
    let hash = path
        .to_lowercase()
        .encode_utf16()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, unit| {
            (hash ^ u64::from(unit)).wrapping_mul(0x0100_0000_01b3)
        });
    format!("QQQ Monitor {hash:016x}")
}

/// The single instance of this executable, held until dropped.
pub struct InstanceLock(Option<HANDLE>);

impl Drop for InstanceLock {
    fn drop(&mut self) {
        if let Some(mutex) = self.0 {
            // SAFETY: the lock holds the only handle this module keeps open and closes it
            // exactly once; releasing a mutex the thread does not own only fails.
            unsafe {
                let _ = ReleaseMutex(mutex);
                let _ = CloseHandle(mutex);
            }
        }
    }
}

/// Becomes the single instance of this executable; `None` while another instance runs. A
/// `replace` start first waits for the running instance to exit.
pub fn claim(replace: bool) -> Option<InstanceLock> {
    claim_as(
        &format!(r"Local\{}", name()),
        replace.then_some(REPLACE_WAIT),
    )
}

fn claim_as(name: &str, replace: Option<Duration>) -> Option<InstanceLock> {
    let name = HSTRING::from(name);
    let deadline = replace.map(|wait| Instant::now() + wait);
    loop {
        // SAFETY: the name outlives the call. The calling thread owns a new mutex until the
        // lock releases it or the thread ends; an existing one only gets a handle.
        let created = unsafe { CreateMutexW(None, true, &name) };
        // SAFETY: reads the calling thread's error of the call above.
        let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        let remaining = deadline.map_or(Duration::ZERO, |deadline| {
            deadline.saturating_duration_since(Instant::now())
        });
        let mutex = match created {
            Ok(mutex) if !existed => return Some(InstanceLock(Some(mutex))),
            Ok(mutex) => mutex,
            // An instance with administrator rights keeps its mutex out of reach.
            Err(error) if error.code() == E_ACCESSDENIED && replace.is_none() => return None,
            Err(error) if error.code() == E_ACCESSDENIED => {
                if remaining.is_zero() {
                    tracing::warn!("等待上一个实例退出超时，照常启动");
                    return Some(InstanceLock(None));
                }
                thread::sleep(POLL.min(remaining));
                continue;
            }
            Err(error) => {
                tracing::warn!(%error, "无法创建单实例互斥体，照常启动");
                return Some(InstanceLock(None));
            }
        };
        if replace.is_none() {
            // SAFETY: closes the handle opened above, which owns nothing.
            let _ = unsafe { CloseHandle(mutex) };
            return None;
        }
        // The running instance owns the mutex until it exits, and the wait hands it over.
        // SAFETY: the handle is live for the call.
        let waited = unsafe { WaitForSingleObject(mutex, remaining.as_millis() as u32) };
        if waited != WAIT_OBJECT_0 && waited != WAIT_ABANDONED {
            tracing::warn!("等待上一个实例退出超时，照常启动");
        }
        return Some(InstanceLock(Some(mutex)));
    }
}

/// Asks the running instance of this executable to show its window, as a click on its
/// notification-area icon does, and lets it come to the foreground.
pub fn activate() {
    let title = HSTRING::from(name());
    let deadline = Instant::now() + FIND_WAIT;
    let window = loop {
        // SAFETY: the class and title outlive the call.
        match unsafe { FindWindowW(tray::CLASS, &title) } {
            Ok(window) => break window,
            Err(_) if Instant::now() < deadline => thread::sleep(POLL),
            Err(_) => return,
        }
    };
    let mut process = 0;
    // SAFETY: `process` outlives the call; a window that is gone yields 0.
    unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
    // SAFETY: plain value arguments; posting to a window that is gone only fails.
    unsafe {
        let _ = AllowSetForegroundWindow(process);
        let _ = PostMessageW(Some(window), tray::OPEN, WPARAM(0), LPARAM(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_follows_the_path_regardless_of_case() {
        let name = name_of(r"\\?\C:\Tools\qqq_monitor.exe");
        assert_eq!(name, name_of(r"\\?\c:\tools\QQQ_Monitor.EXE"));
        assert_ne!(name, name_of(r"\\?\C:\Code\target\debug\qqq_monitor.exe"));
        assert!(name.starts_with("QQQ Monitor "));
    }

    #[test]
    fn a_second_claim_finds_the_instance_and_a_restart_waits_for_it() {
        let name = format!(r"Local\QQQ Monitor test {}", std::process::id());
        let first = claim_as(&name, None).expect("first instance");
        assert!(claim_as(&name, None).is_none());
        let started = Instant::now();
        let restart = {
            let name = name.clone();
            thread::spawn(move || {
                let lock = claim_as(&name, Some(Duration::from_secs(10)));
                (lock.is_some(), started.elapsed())
            })
        };
        thread::sleep(Duration::from_millis(200));
        drop(first);
        let (claimed, elapsed) = restart.join().unwrap();
        assert!(claimed);
        assert!(elapsed >= Duration::from_millis(200));
        assert!(elapsed < Duration::from_secs(5));
        // The restart's lock is gone with its thread, so the name is free again.
        assert!(claim_as(&name, None).is_some());
    }
}
