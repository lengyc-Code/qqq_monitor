//! Notification-area icon and taskbar button of the application window.
//!
//! The icon belongs to a hidden top-level window created on the GUI thread. The message
//! loop the GUI already runs delivers it the icon's clicks, its menu and the
//! `TaskbarCreated` broadcast that follows an Explorer restart. All functions act on the
//! state of the calling thread, so only the GUI thread that called [`attach`] reaches it.

use crate::native::APP_ICON;
use std::{
    cell::{Cell, OnceCell, RefCell},
    mem::size_of,
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute},
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            },
            LibraryLoader::GetModuleHandleW,
        },
        UI::{
            HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi},
            Shell::{
                ITaskbarList, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE,
                NIM_MODIFY, NIM_SETVERSION, NIN_SELECT, NINF_KEY, NOTIFY_ICON_MESSAGE,
                NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW, TaskbarList,
            },
            WindowsAndMessaging::{
                AppendMenuW, ChangeWindowMessageFilterEx, CreatePopupMenu, CreateWindowExW,
                DefWindowProcW, DestroyMenu, HICON, IMAGE_ICON, IsWindowVisible, LR_DEFAULTCOLOR,
                LoadImageW, MF_STRING, MSGFLT_ALLOW, PostMessageW, RegisterClassW,
                RegisterWindowMessageW, SM_CXSMICON, SetForegroundWindow, SetMenuDefaultItem,
                TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
                WM_APP, WM_CLOSE, WM_CONTEXTMENU, WM_NULL, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
            },
        },
    },
    core::{BOOL, HSTRING, PCWSTR, w},
};

/// Message carrying the icon's notifications.
const NOTIFY: u32 = WM_APP + 1;
/// Message that re-applies the icon and the taskbar button after a `TaskbarCreated`
/// broadcast has reached every window of the thread.
const REAPPLY: u32 = WM_APP + 2;
const ICON_ID: u32 = 1;
/// Enter or Space on the focused icon.
const KEY_SELECT: u32 = NIN_SELECT | NINF_KEY;
const MENU_OPEN: usize = 1;
const MENU_EXIT: usize = 2;

/// Icon text: the tooltip and the labels of the "open" and "exit" menu items.
struct Labels {
    tip: String,
    open: String,
    exit: String,
}

struct Tray {
    /// Hidden window that owns the icon.
    window: HWND,
    /// Application window.
    main: HWND,
    icon: HICON,
    taskbar_created: u32,
    /// Present while the icon is shown.
    labels: RefCell<Option<Labels>>,
    /// The application window's taskbar button is removed.
    no_button: Cell<bool>,
    /// The user asked for the application window since the last [`opened`] call.
    opened: Cell<bool>,
    wake: Box<dyn Fn()>,
}

thread_local! {
    static TRAY: OnceCell<Tray> = const { OnceCell::new() };
}

/// Runs `f` on the tray of the calling thread, if it has one. Shared access only, so the
/// window procedure may re-enter while another call waits on the shell.
fn with<R>(f: impl FnOnce(&Tray) -> R) -> Option<R> {
    TRAY.with(|tray| tray.get().map(f))
}

/// Prepares the tray for the application window `main` on the calling GUI thread. `wake` is
/// called when the user asks for the window from the icon, so the GUI polls [`opened`].
/// The icon itself appears with [`show`]. Returns whether the tray is ready.
pub fn attach(main: isize, wake: impl Fn() + 'static) -> bool {
    if with(|_| ()).is_some() {
        return true;
    }
    // SAFETY: a null module name returns the running executable, which outlives the tray.
    let Ok(module) = (unsafe { GetModuleHandleW(PCWSTR::null()) }) else {
        return false;
    };
    let class = w!("QqqMonitorTray");
    let description = WNDCLASSW {
        lpfnWndProc: Some(procedure),
        hInstance: module.into(),
        lpszClassName: class,
        ..Default::default()
    };
    // SAFETY: the class name is a static NUL-terminated string and the procedure lives for
    // the whole process. A zero result leaves window creation below to fail.
    unsafe { RegisterClassW(&description) };
    // SAFETY: the class was registered above with this module; the window is never shown,
    // and a top-level window, unlike a message-only one, receives shell broadcasts.
    let Ok(window) = (unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class,
            w!("QQQ Monitor"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(module.into()),
            None,
        )
    }) else {
        return false;
    };
    // SAFETY: the message name is a static NUL-terminated string.
    let taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
    for message in [NOTIFY, taskbar_created] {
        // Explorer runs at medium integrity; an elevated instance must accept its messages.
        // SAFETY: `window` belongs to this thread; no filter information is requested.
        let _ = unsafe { ChangeWindowMessageFilterEx(window, message, MSGFLT_ALLOW, None) };
    }
    // ITaskbarList needs COM on this thread. The GUI keeps it initialized for the life of
    // the process; an existing apartment of another model serves as well.
    // SAFETY: called on the GUI thread with no reserved pointer.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    TRAY.with(|tray| {
        let _ = tray.set(Tray {
            window,
            main: HWND(main as *mut _),
            icon: small_icon(module.into()),
            taskbar_created,
            labels: RefCell::new(None),
            no_button: Cell::new(false),
            opened: Cell::new(false),
            wake: Box::new(wake),
        });
    });
    true
}

/// The embedded application icon at the size the notification area uses at system DPI.
fn small_icon(module: windows::Win32::Foundation::HINSTANCE) -> HICON {
    // SAFETY: plain value arguments.
    let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForSystem()) };
    // SAFETY: the integer resource ID is passed as MAKEINTRESOURCE. The icon is shown for
    // the rest of the process, so the handle is reclaimed at exit.
    unsafe {
        LoadImageW(
            Some(module),
            PCWSTR(APP_ICON as usize as *const u16),
            IMAGE_ICON,
            size,
            size,
            LR_DEFAULTCOLOR,
        )
    }
    .map_or(HICON::default(), |icon| HICON(icon.0))
}

/// Shows the icon with a tooltip and the labels of its "open" and "exit" menu items, or
/// updates the text of the shown icon.
pub fn show(tip: &str, open: &str, exit: &str) {
    with(|tray| {
        let labels = Labels {
            tip: tip.into(),
            open: open.into(),
            exit: exit.into(),
        };
        if tray.labels.replace(Some(labels)).is_some() {
            tray.notify(NIM_MODIFY);
        } else {
            tray.add();
        }
    });
}

/// Removes the icon.
pub fn hide() {
    with(|tray| {
        if tray.labels.take().is_some() {
            tray.notify(NIM_DELETE);
        }
    });
}

/// Shows or removes the taskbar button of the visible application window. Returns `false`
/// while the window is still hidden, since the shell adds its button once it appears.
pub fn set_taskbar_button(shown: bool) -> bool {
    with(|tray| {
        // SAFETY: only reads the window state; an invalid handle yields false.
        if !unsafe { IsWindowVisible(tray.main) }.as_bool() {
            return false;
        }
        tray.no_button.set(!shown);
        tray.apply_button();
        true
    })
    .unwrap_or(true)
}

/// Cloaks or reveals the application window. The compositor draws nothing of a cloaked
/// window, while its visibility and the rest of its state stay as they are.
pub fn cloak(cloaked: bool) {
    with(|tray| {
        let value = BOOL::from(cloaked);
        // SAFETY: `value` outlives the call and its exact size is given; an invalid handle
        // only fails the call.
        let _ = unsafe {
            DwmSetWindowAttribute(
                tray.main,
                DWMWA_CLOAK,
                (&raw const value).cast(),
                size_of::<BOOL>() as u32,
            )
        };
    });
}

/// Whether the user asked for the application window from the icon since the last call.
pub fn opened() -> bool {
    with(|tray| tray.opened.take()).unwrap_or(false)
}

impl Tray {
    fn data(&self) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.window,
            uID: ICON_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: NOTIFY,
            hIcon: self.icon,
            ..Default::default()
        };
        if let Some(labels) = &*self.labels.borrow() {
            // Leaves the last unit as the terminating NUL.
            let room = data.szTip.len() - 1;
            for (slot, unit) in data.szTip[..room].iter_mut().zip(labels.tip.encode_utf16()) {
                *slot = unit;
            }
        }
        data
    }

    fn notify(&self, message: NOTIFY_ICON_MESSAGE) -> bool {
        let data = self.data();
        // SAFETY: `data` is a fully initialized NOTIFYICONDATAW with its size set, naming
        // this tray's live window and icon.
        unsafe { Shell_NotifyIconW(message, &data) }.as_bool()
    }

    fn add(&self) {
        if self.notify(NIM_ADD) {
            let mut data = self.data();
            data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            // SAFETY: as in `notify`; NIM_SETVERSION reads only the version field.
            let _ = unsafe { Shell_NotifyIconW(NIM_SETVERSION, &data) };
        }
    }

    fn apply_button(&self) {
        // SAFETY: TaskbarList is an in-process server created on this COM thread; the
        // wrapper releases it on drop, and the main window handle is only passed on.
        unsafe {
            let Ok(list) =
                CoCreateInstance::<_, ITaskbarList>(&TaskbarList, None, CLSCTX_INPROC_SERVER)
            else {
                return;
            };
            if list.HrInit().is_err() {
                return;
            }
            let _ = if self.no_button.get() {
                list.DeleteTab(self.main)
            } else {
                list.AddTab(self.main)
            };
        }
    }

    /// Restores the icon and the removed taskbar button after Explorer restarts.
    fn reapply(&self) {
        if self.labels.borrow().is_some() {
            self.add();
        }
        if self.no_button.get() {
            self.apply_button();
        }
    }

    fn open(&self) {
        self.opened.set(true);
        (self.wake)();
    }
}

/// Shows the icon's menu at a screen point and acts on the chosen item.
fn menu(window: HWND, x: i32, y: i32) {
    let Some(Some((open, exit, main))) = with(|tray| {
        let labels = tray.labels.borrow();
        let labels = labels.as_ref()?;
        Some((
            HSTRING::from(&labels.open),
            HSTRING::from(&labels.exit),
            tray.main,
        ))
    }) else {
        return;
    };
    // SAFETY: the menu is created, shown and destroyed here, and the label strings outlive
    // it. TrackPopupMenu runs a modal loop that may re-enter the window procedure, so no
    // tray state is borrowed across it.
    let command = unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let _ = AppendMenuW(menu, MF_STRING, MENU_OPEN, &open);
        let _ = AppendMenuW(menu, MF_STRING, MENU_EXIT, &exit);
        let _ = SetMenuDefaultItem(menu, MENU_OPEN as u32, 0);
        // The menu closes on an outside click only while its owner is in the foreground.
        let _ = SetForegroundWindow(window);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            x,
            y,
            None,
            window,
            None,
        );
        let _ = PostMessageW(Some(window), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        command.0 as usize
    };
    match command {
        MENU_OPEN => {
            with(Tray::open);
        }
        // Closing goes through the window's own close path, which saves the configuration.
        // SAFETY: posting to a window handle has no memory preconditions.
        MENU_EXIT => unsafe {
            let _ = PostMessageW(Some(main), WM_CLOSE, WPARAM(0), LPARAM(0));
        },
        _ => {}
    }
}

/// Window procedure of the hidden tray window.
unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        NOTIFY => {
            // NOTIFYICON_VERSION_4 puts the event in the low word of `lparam` and the anchor
            // point, in screen coordinates, in `wparam`.
            match lparam.0 as u32 & 0xFFFF {
                NIN_SELECT | KEY_SELECT => {
                    with(Tray::open);
                }
                WM_CONTEXTMENU => {
                    let x = wparam.0 as u16 as i16 as i32;
                    let y = (wparam.0 >> 16) as u16 as i16 as i32;
                    menu(window, x, y);
                }
                _ => {}
            }
            LRESULT(0)
        }
        REAPPLY => {
            with(Tray::reapply);
            LRESULT(0)
        }
        _ => {
            if with(|tray| tray.taskbar_created == message) == Some(true) {
                // The application window re-adds its own taskbar button on this broadcast;
                // a posted message runs after it.
                // SAFETY: posting to this thread's own window.
                let _ = unsafe { PostMessageW(Some(window), REAPPLY, WPARAM(0), LPARAM(0)) };
            }
            // SAFETY: forwards the unmodified message of this window.
            unsafe { DefWindowProcW(window, message, wparam, lparam) }
        }
    }
}
