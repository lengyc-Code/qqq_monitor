//! Windows processes. One NtQuerySystemInformation call lists every process with its
//! times, memory, threads and base priority; the command line and owner of a process are
//! read once, when it first appears. Actions reopen the process and confirm its creation
//! time before they touch it.

use crate::native::text;
use qqq_domain::*;
use std::{collections::HashMap, mem::size_of, ptr, time::Instant};
use windows::{
    Wdk::System::{
        SystemInformation::{NtQuerySystemInformation, SystemProcessInformation},
        Threading::{NtQueryInformationProcess, ProcessCommandLineInformation},
    },
    Win32::{
        Foundation::{
            CloseHandle, E_ACCESSDENIED, ERROR_INVALID_PARAMETER, FILETIME, HANDLE,
            STATUS_INFO_LENGTH_MISMATCH, STILL_ACTIVE, UNICODE_STRING,
        },
        Security::{
            GetLengthSid, GetTokenInformation, LookupAccountSidW, SID_NAME_USE, TOKEN_QUERY,
            TOKEN_USER, TokenUser,
        },
        System::{
            Threading::{
                ABOVE_NORMAL_PRIORITY_CLASS, ALL_PROCESSOR_GROUPS, BELOW_NORMAL_PRIORITY_CLASS,
                GetActiveProcessorCount, GetExitCodeProcess, GetProcessTimes, HIGH_PRIORITY_CLASS,
                IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, OpenProcess, OpenProcessToken,
                PROCESS_ACCESS_RIGHTS, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                PROCESS_SET_INFORMATION, PROCESS_TERMINATE, QueryFullProcessImageNameW,
                REALTIME_PRIORITY_CLASS, SetPriorityClass, TerminateProcess,
            },
            WindowsProgramming::SYSTEM_PROCESS_INFORMATION,
        },
    },
    core::{PCWSTR, PWSTR},
};

/// Offsets inside `SYSTEM_PROCESS_INFORMATION::Reserved1`, stable since Windows 7: the
/// private working set, then hard faults, the thread high-water mark and the cycle time,
/// then the creation, user and kernel times.
const PRIVATE_WORKING_SET: usize = 0;
const CREATE_TIME: usize = 24;
const USER_TIME: usize = 32;
const KERNEL_TIME: usize = 40;
/// Largest command line read, in bytes.
const COMMAND_LIMIT: u32 = 64 * 1024;

fn failed(api: &'static str, error: &windows::core::Error) -> Reason {
    if error.code() == E_ACCESSDENIED {
        Reason::AccessDenied
    } else if error.code() == ERROR_INVALID_PARAMETER.to_hresult() {
        Reason::ProcessGone
    } else {
        Reason::CallFailed {
            api,
            code: error.code().0 as u32,
        }
    }
}

/// Process handle closed on drop.
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by this module and is closed exactly once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn open(pid: u32, access: PROCESS_ACCESS_RIGHTS) -> windows::core::Result<Handle> {
    // SAFETY: plain value arguments; the returned handle is owned by `Handle`.
    unsafe { OpenProcess(access, false, pid) }.map(Handle)
}

/// Creation time in 100 ns units, as SYSTEM_PROCESS_INFORMATION reports it.
fn creation(process: &Handle) -> Option<u64> {
    let mut times = [FILETIME::default(); 4];
    let [created, exited, kernel, user] = &mut times;
    // SAFETY: the handle has query access and the four outputs are writable locals.
    unsafe { GetProcessTimes(process.0, created, exited, kernel, user) }.ok()?;
    Some((created.dwHighDateTime as u64) << 32 | created.dwLowDateTime as u64)
}

fn running(process: &Handle) -> bool {
    let mut code = 0_u32;
    // SAFETY: the handle has query access and `code` is a writable local.
    unsafe { GetExitCodeProcess(process.0, &mut code) }.is_ok() && code == STILL_ACTIVE.0 as u32
}

struct Entry {
    pid: u32,
    parent: u32,
    name: String,
    start: u64,
    /// User and kernel time in 100 ns units.
    cpu_time: u64,
    memory: u64,
    threads: u32,
    base_priority: i32,
}

fn reserved(info: &SYSTEM_PROCESS_INFORMATION, offset: usize) -> u64 {
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&info.Reserved1[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

/// UTF-16 text of a UNICODE_STRING whose buffer must lie inside `within`.
fn unicode(value: &UNICODE_STRING, within: &[u8]) -> Option<String> {
    let begin = within.as_ptr() as usize;
    let start = value.Buffer.0 as usize;
    let length = value.Length as usize;
    if length == 0 {
        return Some(String::new());
    }
    if start < begin || start + length > begin + within.len() || !start.is_multiple_of(2) {
        return None;
    }
    // SAFETY: the range was checked to lie inside the live buffer and is 2-byte aligned.
    let units = unsafe { std::slice::from_raw_parts(value.Buffer.0, length / 2) };
    Some(text(units))
}

/// Every process the system reports, except the idle process.
fn entries(buffer: &mut Vec<u64>) -> Result<Vec<Entry>, Reason> {
    for _ in 0..4 {
        let capacity = buffer.len() * size_of::<u64>();
        let mut needed = 0_u32;
        // SAFETY: the buffer is writable for `capacity` bytes and 8-byte aligned, which
        // the entries require.
        let status = unsafe {
            NtQuerySystemInformation(
                SystemProcessInformation,
                buffer.as_mut_ptr().cast(),
                capacity as u32,
                &mut needed,
            )
        };
        if status == STATUS_INFO_LENGTH_MISMATCH {
            // Room for processes that start before the next call.
            buffer.resize((needed as usize + 64 * 1024).div_ceil(size_of::<u64>()), 0);
            continue;
        }
        if status.is_err() {
            return Err(Reason::CallFailed {
                api: "NtQuerySystemInformation",
                code: status.0 as u32,
            });
        }
        // SAFETY: the system initialized `needed` bytes of the buffer, which is u64 storage.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr().cast::<u8>(),
                (needed as usize).min(capacity),
            )
        };
        return parse(bytes);
    }
    Err(Reason::InstancesChanging)
}

fn parse(bytes: &[u8]) -> Result<Vec<Entry>, Reason> {
    let bad = Reason::BadData {
        api: "NtQuerySystemInformation",
    };
    let mut result = Vec::new();
    let mut offset = 0_usize;
    loop {
        if offset + size_of::<SYSTEM_PROCESS_INFORMATION>() > bytes.len() {
            return Err(bad);
        }
        // SAFETY: the entry lies inside the initialized buffer; an unaligned read copies it.
        let info = unsafe {
            ptr::read_unaligned(
                bytes
                    .as_ptr()
                    .add(offset)
                    .cast::<SYSTEM_PROCESS_INFORMATION>(),
            )
        };
        let pid = info.UniqueProcessId.0 as usize as u32;
        if pid != 0 {
            result.push(Entry {
                pid,
                parent: info.Reserved2 as usize as u32,
                name: unicode(&info.ImageName, bytes).ok_or(bad.clone())?,
                start: reserved(&info, CREATE_TIME),
                cpu_time: reserved(&info, USER_TIME).saturating_add(reserved(&info, KERNEL_TIME)),
                memory: reserved(&info, PRIVATE_WORKING_SET),
                threads: info.NumberOfThreads,
                base_priority: info.BasePriority,
            });
        }
        if info.NextEntryOffset == 0 {
            return Ok(result);
        }
        offset += info.NextEntryOffset as usize;
    }
}

/// Priority class of a process base priority.
fn class(base: i32) -> Priority {
    match base {
        ..=4 => Priority::Idle,
        5..=6 => Priority::BelowNormal,
        7..=8 => Priority::Normal,
        9..=10 => Priority::AboveNormal,
        11..=15 => Priority::High,
        _ => Priority::Realtime,
    }
}

fn command_line(process: &Handle) -> Option<String> {
    let mut needed = 0_u32;
    // SAFETY: a zero-length query only reports the required size.
    let _ = unsafe {
        NtQueryInformationProcess(
            process.0,
            ProcessCommandLineInformation,
            ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    if needed == 0 || needed > COMMAND_LIMIT {
        return None;
    }
    let mut buffer = vec![0_u64; (needed as usize).div_ceil(size_of::<u64>())];
    let capacity = buffer.len() * size_of::<u64>();
    // SAFETY: the buffer is writable for `capacity` bytes and aligned for UNICODE_STRING.
    let status = unsafe {
        NtQueryInformationProcess(
            process.0,
            ProcessCommandLineInformation,
            buffer.as_mut_ptr().cast(),
            capacity as u32,
            &mut needed,
        )
    };
    if status.is_err() {
        return None;
    }
    // SAFETY: the buffer is u64 storage of `capacity` bytes that the call initialized.
    let bytes = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), capacity) };
    // SAFETY: a successful call starts the buffer with a UNICODE_STRING.
    let value = unsafe { ptr::read_unaligned(bytes.as_ptr().cast::<UNICODE_STRING>()) };
    unicode(&value, bytes)
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
}

fn image_path(process: &Handle) -> Option<String> {
    let mut buffer = [0_u16; 1024];
    let mut length = buffer.len() as u32;
    // SAFETY: the buffer and its length in characters are valid locals.
    unsafe {
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    }
    .ok()?;
    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

/// SID of the account that owns a process.
fn owner_sid(process: &Handle) -> Option<Vec<u8>> {
    let mut token = HANDLE::default();
    // SAFETY: the process handle has query access and `token` is a writable local.
    unsafe { OpenProcessToken(process.0, TOKEN_QUERY, &mut token) }.ok()?;
    let token = Handle(token);
    let mut length = 0_u32;
    // SAFETY: a query without a buffer only reports the required length.
    let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut length) };
    if length == 0 || length > 4096 {
        return None;
    }
    let mut buffer = vec![0_u64; (length as usize).div_ceil(size_of::<u64>())];
    // SAFETY: the buffer is writable for `length` bytes and aligned for TOKEN_USER.
    unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            length,
            &mut length,
        )
    }
    .ok()?;
    // SAFETY: a successful TokenUser query starts the buffer with TOKEN_USER, whose SID
    // points into the same buffer and stays valid while it lives.
    unsafe {
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let sid = user.User.Sid;
        let size = GetLengthSid(sid) as usize;
        Some(std::slice::from_raw_parts(sid.0.cast::<u8>(), size).to_vec())
    }
}

fn account_name(sid: &[u8]) -> Option<String> {
    let mut name = [0_u16; 256];
    let mut domain = [0_u16; 256];
    let (mut name_length, mut domain_length) = (name.len() as u32, domain.len() as u32);
    let mut kind = SID_NAME_USE::default();
    // SAFETY: `sid` holds a complete SID copied from a token, and both output buffers come
    // with their lengths in characters.
    unsafe {
        LookupAccountSidW(
            PCWSTR::null(),
            windows::Win32::Security::PSID(sid.as_ptr() as *mut _),
            Some(PWSTR(name.as_mut_ptr())),
            &mut name_length,
            Some(PWSTR(domain.as_mut_ptr())),
            &mut domain_length,
            &mut kind,
        )
    }
    .ok()?;
    Some(text(&name))
}

/// Command line and owner of one instance.
struct Details {
    command: String,
    user: String,
}

pub struct WindowsProcesses {
    buffer: Vec<u64>,
    details: HashMap<(u32, u64), Details>,
    /// Account names by SID.
    users: HashMap<Vec<u8>, String>,
    processors: u32,
    epoch: Instant,
}

impl WindowsProcesses {
    pub fn new() -> Self {
        // SAFETY: plain value argument.
        let processors = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) }.max(1);
        Self {
            buffer: vec![0; 128 * 1024],
            details: HashMap::new(),
            users: HashMap::new(),
            processors,
            epoch: Instant::now(),
        }
    }

    fn details(&mut self, entry: &Entry) -> Details {
        let Ok(process) = open(entry.pid, PROCESS_QUERY_LIMITED_INFORMATION) else {
            return Details {
                command: String::new(),
                user: String::new(),
            };
        };
        // The PID may already belong to a newer process.
        if creation(&process) != Some(entry.start) {
            return Details {
                command: String::new(),
                user: String::new(),
            };
        }
        let command = command_line(&process)
            .or_else(|| image_path(&process))
            .unwrap_or_default();
        let user = owner_sid(&process)
            .and_then(|sid| {
                if let Some(name) = self.users.get(&sid) {
                    return Some(name.clone());
                }
                let name = account_name(&sid)?;
                self.users.insert(sid, name.clone());
                Some(name)
            })
            .unwrap_or_default();
        Details { command, user }
    }
}

impl ProcessSource for WindowsProcesses {
    fn snapshot(&mut self) -> Result<ProcessSnapshot, Reason> {
        let clock = self.epoch.elapsed().as_secs_f64();
        let mut buffer = std::mem::take(&mut self.buffer);
        let entries = entries(&mut buffer);
        self.buffer = buffer;
        let entries = entries?;
        let mut details = HashMap::with_capacity(entries.len());
        let mut processes = Vec::with_capacity(entries.len());
        for entry in entries {
            let key = (entry.pid, entry.start);
            let detail = match self.details.remove(&key) {
                Some(detail) => detail,
                None => self.details(&entry),
            };
            processes.push(ProcessInfo {
                id: ProcessId {
                    host: ProcessHost::Windows,
                    pid: entry.pid,
                    start: entry.start,
                },
                parent: (entry.parent != 0).then_some(entry.parent),
                name: entry.name,
                command: detail.command.clone(),
                user: detail.user.clone(),
                cpu_time: entry.cpu_time as f64 / 10_000_000.0,
                memory: entry.memory,
                threads: entry.threads,
                priority: Some(class(entry.base_priority)),
            });
            details.insert(key, detail);
        }
        self.details = details;
        Ok(ProcessSnapshot {
            clock,
            processors: self.processors,
            processes,
        })
    }
}

/// Ends or reprioritizes the instance `pid` created at `start`.
pub fn act(pid: u32, start: u64, action: ProcessAction) -> Result<(), Reason> {
    let access = match action {
        ProcessAction::End | ProcessAction::Kill => PROCESS_TERMINATE,
        ProcessAction::SetPriority(_) => PROCESS_SET_INFORMATION,
    } | PROCESS_QUERY_LIMITED_INFORMATION;
    let process = open(pid, access).map_err(|e| failed("OpenProcess", &e))?;
    if creation(&process) != Some(start) || !running(&process) {
        return Err(Reason::ProcessGone);
    }
    match action {
        // SAFETY: the handle was opened with PROCESS_TERMINATE for the confirmed instance.
        ProcessAction::End | ProcessAction::Kill => {
            unsafe { TerminateProcess(process.0, 1) }.map_err(|e| failed("TerminateProcess", &e))
        }
        ProcessAction::SetPriority(priority) => {
            let class = match priority {
                Priority::Idle => IDLE_PRIORITY_CLASS,
                Priority::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
                Priority::Normal => NORMAL_PRIORITY_CLASS,
                Priority::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
                Priority::High => HIGH_PRIORITY_CLASS,
                Priority::Realtime => REALTIME_PRIORITY_CLASS,
            };
            // SAFETY: the handle was opened with PROCESS_SET_INFORMATION for the confirmed
            // instance.
            unsafe { SetPriorityClass(process.0, class) }
                .map_err(|e| failed("SetPriorityClass", &e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_this_process_with_its_command_line() {
        let mut source = WindowsProcesses::new();
        let snapshot = source.snapshot().unwrap();
        let own = std::process::id();
        let me = snapshot
            .processes
            .iter()
            .find(|p| p.id.pid == own)
            .expect("own process listed");
        assert!(me.name.ends_with(".exe"));
        assert!(!me.command.is_empty());
        assert!(!me.user.is_empty());
        assert!(me.memory > 0 && me.threads > 0);
        assert!(snapshot.processes.iter().all(|p| p.id.pid != 0));
        assert!(snapshot.processors >= 1);
    }

    #[test]
    fn acts_only_on_the_confirmed_instance() {
        use std::process::{Command, Stdio};
        let mut child = Command::new("ping")
            .args(["-n", "30", "127.0.0.1"])
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        let mut source = WindowsProcesses::new();
        let find = |source: &mut WindowsProcesses| {
            source
                .snapshot()
                .unwrap()
                .processes
                .into_iter()
                .find(|p| p.id.pid == pid)
                .unwrap()
        };
        let start = find(&mut source).id.start;
        assert_eq!(
            act(pid, start + 1, ProcessAction::End),
            Err(Reason::ProcessGone)
        );
        act(pid, start, ProcessAction::SetPriority(Priority::Idle)).unwrap();
        assert_eq!(find(&mut source).priority, Some(Priority::Idle));
        act(pid, start, ProcessAction::End).unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(1));
        assert_eq!(
            act(pid, start, ProcessAction::End),
            Err(Reason::ProcessGone)
        );
    }

    #[test]
    fn base_priorities_map_to_their_classes() {
        for (base, priority) in [
            (4, Priority::Idle),
            (6, Priority::BelowNormal),
            (8, Priority::Normal),
            (10, Priority::AboveNormal),
            (13, Priority::High),
            (24, Priority::Realtime),
        ] {
            assert_eq!(class(base), priority);
        }
    }
}
