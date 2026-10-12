//! Start at sign-in: a value under the current user's `Run` registry key holds the quoted
//! path of this executable followed by [`ARGUMENT`]. Task Manager and the Settings app switch such an entry off with
//! a marker of the same name under `StartupApproved\Run`; an entry switched off there counts
//! as off, and switching it on here removes the marker.

use crate::native::text;
use qqq_domain::Reason;
use std::os::windows::ffi::OsStrExt;
use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR},
        System::Registry::{
            HKEY_CURRENT_USER, REG_ROUTINE_FLAGS, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_SZ,
            RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
        },
    },
    core::{HSTRING, PCWSTR, w},
};

const RUN: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const APPROVED: PCWSTR =
    w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run");
/// Name of the value in both keys.
const NAME: &str = "QQQ Monitor";
/// Argument the executable receives when Windows starts it at sign-in.
pub const ARGUMENT: &str = "--autostart";

/// Whether Windows starts this executable when the user signs in.
pub fn enabled() -> bool {
    enabled_as(&HSTRING::from(NAME))
}

/// Adds the entry that starts this executable at sign-in, or removes it.
pub fn set(enabled: bool) -> Result<(), Reason> {
    set_as(&HSTRING::from(NAME), enabled)
}

fn failed(api: &'static str, status: WIN32_ERROR) -> Reason {
    Reason::CallFailed {
        api,
        code: status.0,
    }
}

/// Command line that starts this executable, as UTF-16 with a terminating NUL.
fn command() -> Result<Vec<u16>, Reason> {
    let path = std::env::current_exe().map_err(|error| Reason::CallFailed {
        api: "GetModuleFileNameW",
        code: error.raw_os_error().unwrap_or_default() as u32,
    })?;
    let quote = u16::from(b'"');
    Ok(std::iter::once(quote)
        .chain(path.as_os_str().encode_wide())
        .chain([quote, u16::from(b' ')])
        .chain(ARGUMENT.encode_utf16())
        .chain([0])
        .collect())
}

/// Data of a value under a key of the current user; `None` when the value is missing or of
/// another type than `flags` allow.
fn value(key: PCWSTR, name: &HSTRING, flags: REG_ROUTINE_FLAGS) -> Option<Vec<u8>> {
    let mut size = 0_u32;
    // SAFETY: the key and value names are NUL-terminated and outlive the call; only the
    // size is written, to a valid local.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            name,
            flags,
            None,
            None,
            Some(&mut size),
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let mut data = vec![0_u8; size as usize];
    // SAFETY: as above, and `data` holds the `size` writable bytes the call is told of.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            name,
            flags,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    data.truncate(size as usize);
    (status == ERROR_SUCCESS).then_some(data)
}

/// Removes a value under a key of the current user; a missing value or key counts as removed.
fn delete(key: PCWSTR, name: &HSTRING) -> Result<(), Reason> {
    // SAFETY: the key and value names are NUL-terminated and outlive the call.
    match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key, name) } {
        ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
        status => Err(failed("RegDeleteKeyValueW", status)),
    }
}

fn enabled_as(name: &HSTRING) -> bool {
    let (Ok(command), Some(data)) = (command(), value(RUN, name, RRF_RT_REG_SZ)) else {
        return false;
    };
    let entry: Vec<u16> = data
        .as_chunks()
        .0
        .iter()
        .copied()
        .map(u16::from_ne_bytes)
        .collect();
    // Windows paths ignore case. The first byte of the marker is odd while the entry is off.
    text(&entry).to_lowercase() == text(&command).to_lowercase()
        && value(APPROVED, name, RRF_RT_REG_BINARY)
            .is_none_or(|marker| marker.first().is_none_or(|flags| flags & 1 == 0))
}

fn set_as(name: &HSTRING, enabled: bool) -> Result<(), Reason> {
    if enabled {
        let command = command()?;
        // SAFETY: the key and value names are NUL-terminated and outlive the call, and
        // `command` holds the byte count given, including its NUL; the call copies it.
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN,
                name,
                REG_SZ.0,
                Some(command.as_ptr().cast()),
                (command.len() * size_of::<u16>()) as u32,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(failed("RegSetKeyValueW", status));
        }
    } else {
        delete(RUN, name)?;
    }
    // A marker left from an entry switched off would keep a new entry off.
    delete(APPROVED, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Registry::REG_BINARY;

    #[test]
    fn entry_round_trip_and_marker_switches_it_off() {
        let name = HSTRING::from(format!("QQQ Monitor test {}", std::process::id()));
        assert!(!enabled_as(&name));
        set_as(&name, true).unwrap();
        assert!(enabled_as(&name));
        // The marker Task Manager writes when it switches an entry off.
        let marker = [3_u8, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8];
        // SAFETY: the key and value names are NUL-terminated and `marker` holds the byte
        // count given.
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                APPROVED,
                &name,
                REG_BINARY.0,
                Some(marker.as_ptr().cast()),
                marker.len() as u32,
            )
        };
        assert_eq!(status, ERROR_SUCCESS);
        assert!(!enabled_as(&name));
        set_as(&name, true).unwrap();
        assert!(enabled_as(&name));
        set_as(&name, false).unwrap();
        assert!(!enabled_as(&name));
        assert_eq!(value(RUN, &name, RRF_RT_REG_SZ), None);
        assert_eq!(value(APPROVED, &name, RRF_RT_REG_BINARY), None);
        set_as(&name, false).unwrap();
    }
}
