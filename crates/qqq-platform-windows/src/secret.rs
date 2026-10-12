//! Passwords and passphrases in the Windows Credential Manager, as generic credentials of
//! the signed-in user kept on this computer. The credential blob holds the secret as UTF-8.

use qqq_domain::{Reason, SecretStore};
use std::ptr;
use windows::{
    Win32::{
        Foundation::ERROR_NOT_FOUND,
        Security::Credentials::{
            CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree,
            CredReadW, CredWriteW,
        },
    },
    core::{HSTRING, PWSTR},
};

pub struct Credentials;

/// A credential returned by `CredReadW`, freed once.
struct Credential(*mut CREDENTIALW);
impl Drop for Credential {
    fn drop(&mut self) {
        // SAFETY: the pointer came from a successful CredReadW and is freed exactly once.
        unsafe { CredFree(self.0.cast()) };
    }
}

fn failed(api: &'static str, error: &windows::core::Error) -> Reason {
    Reason::CallFailed {
        api,
        code: error.code().0 as u32,
    }
}

impl SecretStore for Credentials {
    fn secret(&self, name: &str) -> Option<String> {
        let target = HSTRING::from(name);
        let mut credential = ptr::null_mut();
        // SAFETY: the target is a NUL-terminated string that outlives the call, and the
        // out pointer is a valid local.
        unsafe { CredReadW(&target, CRED_TYPE_GENERIC, None, &mut credential) }.ok()?;
        let credential = Credential(credential);
        // SAFETY: CredReadW succeeded, so the credential and its blob of
        // `CredentialBlobSize` bytes stay valid until `credential` is dropped.
        let blob = unsafe {
            let value = &*credential.0;
            if value.CredentialBlob.is_null() {
                Vec::new()
            } else {
                std::slice::from_raw_parts(value.CredentialBlob, value.CredentialBlobSize as usize)
                    .to_vec()
            }
        };
        String::from_utf8(blob).ok()
    }

    fn set_secret(&self, name: &str, secret: Option<&str>) -> Result<(), Reason> {
        let Some(secret) = secret else {
            let target = HSTRING::from(name);
            // SAFETY: the target is a NUL-terminated string that outlives the call.
            return match unsafe { CredDeleteW(&target, CRED_TYPE_GENERIC, None) } {
                Ok(()) => Ok(()),
                Err(error) if error.code() == ERROR_NOT_FOUND.to_hresult() => Ok(()),
                Err(error) => Err(failed("CredDeleteW", &error)),
            };
        };
        let mut target: Vec<u16> = name.encode_utf16().chain([0]).collect();
        let mut blob = secret.as_bytes().to_vec();
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        // SAFETY: the target name and blob buffers outlive the call, and the blob size is
        // their exact length; CredWriteW copies both.
        let result = unsafe { CredWriteW(&credential, 0) };
        blob.fill(0);
        result.map_err(|error| failed("CredWriteW", &error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_round_trip_and_removal_is_idempotent() {
        let name = format!("qqq_monitor/test/{}", std::process::id());
        let store = Credentials;
        store.set_secret(&name, Some("pässwörd 密码")).unwrap();
        assert_eq!(store.secret(&name).as_deref(), Some("pässwörd 密码"));
        store.set_secret(&name, Some("second")).unwrap();
        assert_eq!(store.secret(&name).as_deref(), Some("second"));
        store.set_secret(&name, None).unwrap();
        assert_eq!(store.secret(&name), None);
        store.set_secret(&name, None).unwrap();
    }
}
