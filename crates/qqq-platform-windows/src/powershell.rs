//! PowerShell sessions that quick commands start.
//!
//! Each session is a Windows PowerShell process of its own. The script travels as
//! `-EncodedCommand`, so quotes and other special characters in it need no escaping. The
//! process starts suspended and joins a job object of its own before it runs, so the job
//! holds every process the session starts, and terminating the job ends them all.

use qqq_domain::{Reason, ShellSession};
use std::{mem::size_of, path::PathBuf};
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_DIRECTORY, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, GetLastError,
            HANDLE, WAIT_TIMEOUT,
        },
        System::{
            JobObjects::{AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject},
            Threading::{
                CREATE_NEW_CONSOLE, CREATE_NO_WINDOW, CREATE_SUSPENDED, CreateProcessW,
                PROCESS_INFORMATION, ResumeThread, STARTUPINFOW, TerminateProcess,
                WaitForSingleObject,
            },
        },
    },
    core::{HSTRING, PCWSTR, PWSTR},
};

/// Exit code of the processes of a terminated session.
const TERMINATED: u32 = 1;

/// Handle closed on drop.
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle was created by this module and is closed exactly once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// A started session: its PowerShell process, and the job that holds it with every process
/// started from it.
struct Session {
    process: Handle,
    /// `None` when the system kept the process out of a job of its own; terminating then
    /// ends the PowerShell process alone.
    job: Option<Handle>,
}

impl ShellSession for Session {
    fn running(&self) -> bool {
        // SAFETY: a zero-length wait on the session's own process handle.
        let state = unsafe { WaitForSingleObject(self.process.0, 0) };
        state == WAIT_TIMEOUT
    }

    fn terminate(&self) -> Result<(), Reason> {
        // SAFETY: both handles belong to this session and stay open during the call.
        let (api, result) = match &self.job {
            Some(job) => ("TerminateJobObject", unsafe {
                TerminateJobObject(job.0, TERMINATED)
            }),
            None => ("TerminateProcess", unsafe {
                TerminateProcess(self.process.0, TERMINATED)
            }),
        };
        match result {
            // A session that ended by itself meanwhile is terminated all the same.
            Err(_) if !self.running() => Ok(()),
            Err(error) => Err(Reason::CallFailed {
                api,
                code: error.code().0 as u32,
            }),
            Ok(()) => Ok(()),
        }
    }
}

/// Windows PowerShell in the system folder, which every Windows installation has.
fn executable() -> PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    PathBuf::from(root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe")
}

/// Base64 of a script in UTF-16LE, the form `-EncodedCommand` reads.
fn encode(script: &str) -> String {
    const DIGITS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let group = chunk.iter().enumerate().fold(0u32, |group, (i, byte)| {
            group | (u32::from(*byte) << (16 - 8 * i))
        });
        for i in 0..4 {
            if i <= chunk.len() {
                text.push(DIGITS[((group >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                text.push('=');
            }
        }
    }
    text
}

/// Starts a PowerShell session that runs `script` in `directory`, or in the user's profile
/// folder when it is empty. A visible session opens a console window titled `title` that
/// stays open after the script ends; a hidden one has no window and ends with the script.
pub fn start(
    script: &str,
    title: &str,
    visible: bool,
    directory: &str,
) -> Result<Box<dyn ShellSession>, Reason> {
    let program = HSTRING::from(executable().as_os_str());
    let options = if visible {
        "-NoExit -NoLogo"
    } else {
        "-NoLogo -NonInteractive"
    };
    let line = format!("\"{program}\" {options} -EncodedCommand {}", encode(script));
    let mut line: Vec<u16> = line.encode_utf16().chain([0]).collect();
    let directory = match directory.trim() {
        "" => std::env::var_os("USERPROFILE").map(HSTRING::from),
        folder => Some(HSTRING::from(folder)),
    };
    let title = HSTRING::from(title);
    let startup = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        lpTitle: if visible {
            PWSTR(title.as_ptr().cast_mut())
        } else {
            PWSTR::null()
        },
        ..Default::default()
    };
    let window = if visible {
        CREATE_NEW_CONSOLE
    } else {
        CREATE_NO_WINDOW
    };
    // SAFETY: an unnamed job with default security; `Handle` owns the result.
    let job = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
        .ok()
        .map(Handle);
    let mut process = PROCESS_INFORMATION::default();
    // SAFETY: the strings and structures are locals that outlive the call; the command line
    // is a writable NUL-terminated buffer as the call requires, and the console title is
    // only read. No handles are inherited, so the session opens its own console rather than
    // writing to this process's.
    unsafe {
        CreateProcessW(
            &program,
            Some(PWSTR(line.as_mut_ptr())),
            None,
            None,
            false,
            window | CREATE_SUSPENDED,
            None,
            directory
                .as_ref()
                .map_or(PCWSTR::null(), |folder| PCWSTR(folder.as_ptr())),
            &startup,
            &mut process,
        )
    }
    .map_err(|error| {
        let code = error.code();
        if [ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_DIRECTORY]
            .iter()
            .any(|e| e.to_hresult() == code)
        {
            Reason::PathNotFound
        } else {
            Reason::CallFailed {
                api: "CreateProcessW",
                code: code.0 as u32,
            }
        }
    })?;
    let thread = Handle(process.hThread);
    let process = Handle(process.hProcess);
    // A process the system keeps out of the job still starts; terminating it then reaches
    // the PowerShell process alone.
    // SAFETY: both handles are open, and the process has not run yet.
    let job = job.filter(|job| unsafe { AssignProcessToJobObject(job.0, process.0) }.is_ok());
    // SAFETY: resumes the suspended main thread of the process just created.
    if unsafe { ResumeThread(thread.0) } == u32::MAX {
        // SAFETY: reads this thread's last error, then ends the process that never ran.
        let code = unsafe { GetLastError() }.0;
        let _ = unsafe { TerminateProcess(process.0, TERMINATED) };
        return Err(Reason::CallFailed {
            api: "ResumeThread",
            code,
        });
    }
    Ok(Box::new(Session { process, job }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use windows::Win32::{
        Foundation::WAIT_OBJECT_0,
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE},
    };

    #[test]
    fn scripts_are_encoded_as_utf16_base64() {
        assert_eq!(encode(""), "");
        assert_eq!(encode("d"), "ZAA=");
        assert_eq!(encode("dir"), "ZABpAHIA");
        assert_eq!(encode("Get-Date"), "RwBlAHQALQBEAGEAdABlAA==");
        assert_eq!(encode("echo '中'"), "ZQBjAGgAbwAgACcALU4nAA==");
    }

    #[test]
    fn terminating_a_session_ends_every_process_it_started() {
        let marker = std::env::temp_dir().join(format!("qqq-session-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let script = format!(
            "$child = Start-Process -FilePath powershell.exe -ArgumentList '-NoProfile','-Command','Start-Sleep 60' -WindowStyle Hidden -PassThru\nSet-Content -LiteralPath '{}' -Value $child.Id\nStart-Sleep 60",
            marker.display()
        );
        let session = start(&script, "", false, "").unwrap();
        assert!(session.running());
        let deadline = Instant::now() + Duration::from_secs(30);
        let child = loop {
            let pid = std::fs::read_to_string(&marker)
                .ok()
                .and_then(|text| text.trim().parse::<u32>().ok());
            if let Some(pid) = pid {
                break pid;
            }
            assert!(Instant::now() < deadline, "the session started no child");
            std::thread::sleep(Duration::from_millis(100));
        };
        let _ = std::fs::remove_file(&marker);
        // SAFETY: plain value arguments; `Handle` owns the result.
        let child = Handle(unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, child) }.unwrap());
        session.terminate().unwrap();
        // SAFETY: waits on a handle this test owns.
        assert_eq!(
            unsafe { WaitForSingleObject(child.0, 10_000) },
            WAIT_OBJECT_0
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while session.running() {
            assert!(Instant::now() < deadline, "the session kept running");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(session.terminate(), Ok(()));
    }
}
