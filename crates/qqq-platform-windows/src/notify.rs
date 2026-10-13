//! Toast notifications. Windows shows the toasts of a desktop application whose
//! application user model ID sits on a shortcut in the Start menu, and takes their name
//! and icon from it: the shortcut `QQQ Monitor` in the current user's Start menu programs
//! points to this executable and carries the ID. [`prepare`] writes it once per run,
//! before the first toast. Clicking a toast shows the main window.

use crate::tray;
use qqq_domain::Reason;
use std::{
    collections::VecDeque,
    mem::ManuallyDrop,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
use windows::{
    Data::Xml::Dom::XmlDocument,
    Foundation::TypedEventHandler,
    UI::Notifications::{NotificationSetting, ToastNotification, ToastNotificationManager},
    Win32::{
        Foundation::PROPERTYKEY,
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
                CoTaskMemFree, IPersistFile, StructuredStorage::PROPVARIANT,
            },
            Variant::VT_LPWSTR,
        },
        UI::Shell::{
            FOLDERID_Programs, IShellLinkW, KF_FLAG_DEFAULT, PropertiesSystem::IPropertyStore,
            SHGetKnownFolderPath, ShellLink,
        },
    },
    core::{GUID, HSTRING, Interface, PWSTR},
};

/// Application user model ID of the toasts.
const APP_ID: &str = "QQQMonitor";
/// Name of the Start menu shortcut, which the toasts show.
const SHORTCUT: &str = "QQQ Monitor.lnk";
/// `System.AppUserModel.ID`.
const APP_ID_KEY: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
    pid: 5,
};
/// Toasts kept alive so that a click still reaches their handler.
const KEPT: usize = 8;

fn failed(api: &'static str, error: windows::core::Error) -> Reason {
    Reason::CallFailed {
        api,
        code: error.code().0 as u32,
    }
}

/// The current user's Start menu programs folder.
fn programs() -> Result<PathBuf, Reason> {
    // SAFETY: the folder ID is a static GUID; the returned string is copied and then freed
    // once with the COM allocator that allocated it.
    unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_DEFAULT, None)
            .map_err(|e| failed("SHGetKnownFolderPath", e))?;
        let text = path.to_string();
        CoTaskMemFree(Some(path.0.cast()));
        text.map(PathBuf::from).map_err(|_| Reason::BadData {
            api: "SHGetKnownFolderPath",
        })
    }
}

/// Writes the Start menu shortcut that gives this executable the toasts' ID.
fn register() -> Result<(), Reason> {
    let target = std::env::current_exe().map_err(|error| Reason::CallFailed {
        api: "GetModuleFileNameW",
        code: error.raw_os_error().unwrap_or_default() as u32,
    })?;
    let path = programs()?.join(SHORTCUT);
    let id: Vec<u16> = APP_ID.encode_utf16().chain([0]).collect();
    // SAFETY: COM objects are created on this thread, which has joined an apartment, and
    // released by their wrappers. The string variant borrows `id`, which outlives the
    // store's copy of it; it is never dropped, since clearing it would free `id`.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| failed("CoCreateInstance", e))?;
        link.SetPath(&HSTRING::from(target.as_os_str()))
            .map_err(|e| failed("IShellLinkW.SetPath", e))?;
        if let Some(directory) = target.parent() {
            link.SetWorkingDirectory(&HSTRING::from(directory.as_os_str()))
                .map_err(|e| failed("IShellLinkW.SetWorkingDirectory", e))?;
        }
        let store: IPropertyStore = link.cast().map_err(|e| failed("IPropertyStore", e))?;
        let mut value = ManuallyDrop::new(PROPVARIANT::default());
        let inner = &mut *value.Anonymous.Anonymous;
        inner.vt = VT_LPWSTR;
        inner.Anonymous.pwszVal = PWSTR(id.as_ptr().cast_mut());
        store
            .SetValue(&APP_ID_KEY, &*value)
            .map_err(|e| failed("IPropertyStore.SetValue", e))?;
        store
            .Commit()
            .map_err(|e| failed("IPropertyStore.Commit", e))?;
        let file: IPersistFile = link.cast().map_err(|e| failed("IPersistFile", e))?;
        file.Save(&HSTRING::from(path.as_os_str()), true)
            .map_err(|e| failed("IPersistFile.Save", e))
    }
}

/// Makes sure the toasts' Start menu shortcut is in place, once per run. Windows may take a
/// moment to notice a new shortcut, so callers write it ahead of the first toast.
pub fn prepare() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        // The Windows Runtime and the shell link need COM on the calling thread; a thread
        // that already joined an apartment keeps it.
        // SAFETY: no reserved pointer; the thread stays initialized for its lifetime.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if let Err(reason) = register() {
            tracing::warn!(?reason, "无法创建通知所需的开始菜单快捷方式");
        }
    });
}

/// `text` with the characters XML reserves escaped.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            c => escaped.push(c),
        }
    }
    escaped
}

/// Shows a toast with a title and a body. Fails with [`Reason::NotificationsOff`] while the
/// user has switched the application's notifications off.
pub fn notify(title: &str, body: &str) -> Result<(), Reason> {
    static TOASTS: Mutex<VecDeque<ToastNotification>> = Mutex::new(VecDeque::new());
    prepare();
    // SAFETY: as in `prepare`, for a thread other than the one that registered.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let xml = format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        escape(title),
        escape(body)
    );
    let document = XmlDocument::new().map_err(|e| failed("XmlDocument", e))?;
    document
        .LoadXml(&HSTRING::from(xml))
        .map_err(|e| failed("XmlDocument.LoadXml", e))?;
    let toast = ToastNotification::CreateToastNotification(&document)
        .map_err(|e| failed("CreateToastNotification", e))?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))
        .map_err(|e| failed("CreateToastNotifierWithId", e))?;
    if notifier
        .Setting()
        .is_ok_and(|setting| setting != NotificationSetting::Enabled)
    {
        return Err(Reason::NotificationsOff);
    }
    toast
        .Activated(&TypedEventHandler::new(|_, _| {
            tray::request_open();
            Ok(())
        }))
        .map_err(|e| failed("ToastNotification.Activated", e))?;
    notifier
        .Show(&toast)
        .map_err(|e| failed("ToastNotifier.Show", e))?;
    let mut toasts = TOASTS.lock().unwrap_or_else(|poison| poison.into_inner());
    toasts.push_back(toast);
    if toasts.len() > KEPT {
        toasts.pop_front();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shows a real toast, which then waits in the notification center.
    #[test]
    #[ignore = "shows a notification on the desktop"]
    fn toast_reaches_the_notification_center() {
        notify("QQQ Monitor", "测试通知 <&> テスト").unwrap();
        let history = ToastNotificationManager::History().unwrap();
        let toasts = history.GetHistoryWithId(&HSTRING::from(APP_ID)).unwrap();
        assert!(toasts.Size().unwrap() > 0);
    }

    #[test]
    fn reserved_characters_are_escaped() {
        assert_eq!(
            escape(r#"<C:\ & "D:">'"#),
            "&lt;C:\\ &amp; &quot;D:&quot;&gt;&apos;"
        );
    }
}
