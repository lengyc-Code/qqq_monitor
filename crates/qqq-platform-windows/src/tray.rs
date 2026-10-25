//! Notification-area icon with its balloons, taskbar button of the application window, and
//! the screenshot, note and quick command shortcuts.
//!
//! The icon belongs to a hidden top-level window created on the GUI thread. The message
//! loop the GUI already runs delivers it the icon's clicks, its menu, the
//! `TaskbarCreated` broadcast that follows an Explorer restart, the shortcuts and
//! clipboard changes. Most functions act on the state of the calling thread, so only
//! the GUI thread that called [`attach`] reaches it; [`post`] and [`request_open`] work
//! from any thread. What the GUI is asked for or told arrives in one queue of
//! [`DesktopEvent`]s, which [`take_events`] empties.

use crate::{clipboard, instance, native::APP_ICON};
use qqq_domain::{DesktopEvent, Hotkey, HotkeyKey, Reason, Shortcut};
use std::{
    cell::{Cell, OnceCell, RefCell},
    mem::size_of,
    sync::{
        Mutex,
        atomic::{AtomicIsize, Ordering},
    },
};
use windows::{
    Win32::{
        Foundation::{
            ERROR_HOTKEY_ALREADY_REGISTERED, ERROR_INVALID_WINDOW_HANDLE, HWND, LPARAM, LRESULT,
            WPARAM,
        },
        Graphics::Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute},
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            },
            LibraryLoader::GetModuleHandleW,
        },
        UI::{
            HiDpi::{GetDpiForSystem, GetSystemMetricsForDpi},
            Input::KeyboardAndMouse::{
                MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey,
            },
            Shell::{
                ITaskbarList, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIIF_NOSOUND,
                NIIF_RESPECT_QUIET_TIME, NIIF_USER, NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY,
                NIM_SETVERSION, NIN_BALLOONUSERCLICK, NIN_SELECT, NINF_KEY, NOTIFY_ICON_MESSAGE,
                NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW, TaskbarList,
            },
            WindowsAndMessaging::{
                AppendMenuW, ChangeWindowMessageFilterEx, CreatePopupMenu, CreateWindowExW,
                DefWindowProcW, DestroyMenu, HICON, IMAGE_ICON, IsWindowVisible, KillTimer,
                LR_DEFAULTCOLOR, LoadImageW, MF_SEPARATOR, MF_STRING, MSGFLT_ALLOW, PostMessageW,
                RegisterClassW, RegisterWindowMessageW, SM_CXSMICON, SetForegroundWindow,
                SetMenuDefaultItem, SetTimer, TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD,
                TPM_RIGHTBUTTON, TrackPopupMenu, WM_APP, WM_CLIPBOARDUPDATE, WM_CLOSE,
                WM_CONTEXTMENU, WM_HOTKEY, WM_NULL, WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW,
                WS_POPUP,
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
/// Message from another thread, or another start of this executable, asking for the
/// application window.
pub(crate) const OPEN: u32 = WM_APP + 3;
/// Window class of the hidden tray window, whose title is the instance name.
pub(crate) const CLASS: PCWSTR = w!("QqqMonitorTray");
/// Message from another thread that posted events.
const EVENTS: u32 = WM_APP + 4;
const ICON_ID: u32 = 1;
/// Enter or Space on the focused icon.
const KEY_SELECT: u32 = NIN_SELECT | NINF_KEY;
const MENU_OPEN: usize = 1;
const MENU_EXIT: usize = 2;
const MENU_SCREENSHOT: usize = 3;
const MENU_NOTE: usize = 4;
/// Shortcut identity of the first quick command; the others follow it in order.
const COMMAND_HOTKEYS: i32 = 0x100;
/// Timer that asks for a screenshot once the icon's menu has faded from the screen.
const SCREENSHOT_TIMER: usize = 1;
const MENU_FADE_MS: u32 = 250;

/// Icon text: the tooltip and the labels of the "open", "screenshot", "note" and "exit"
/// menu items.
struct Labels {
    tip: String,
    open: String,
    screenshot: String,
    note: String,
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
    /// Events of the GUI thread since the last [`take_events`].
    events: RefCell<Vec<DesktopEvent>>,
    wake: Box<dyn Fn()>,
}

thread_local! {
    static TRAY: OnceCell<Tray> = const { OnceCell::new() };
}

/// Hidden tray window, for threads other than the GUI thread to post to.
static WINDOW: AtomicIsize = AtomicIsize::new(0);
/// Events of other threads since the last [`take_events`].
static POSTED: Mutex<Vec<DesktopEvent>> = Mutex::new(Vec::new());

/// Runs `f` on the tray of the calling thread, if it has one. Shared access only, so the
/// window procedure may re-enter while another call waits on the shell.
fn with<R>(f: impl FnOnce(&Tray) -> R) -> Option<R> {
    TRAY.with(|tray| tray.get().map(f))
}

/// Prepares the tray for the application window `main` on the calling GUI thread. `wake` is
/// called whenever events arrive, so the GUI takes them with [`take_events`]. The icon
/// itself appears with [`show`]. Returns whether the tray is ready.
pub fn attach(main: isize, wake: impl Fn() + 'static) -> bool {
    if with(|_| ()).is_some() {
        return true;
    }
    // SAFETY: a null module name returns the running executable, which outlives the tray.
    let Ok(module) = (unsafe { GetModuleHandleW(PCWSTR::null()) }) else {
        return false;
    };
    let class = CLASS;
    let description = WNDCLASSW {
        lpfnWndProc: Some(procedure),
        hInstance: module.into(),
        lpszClassName: class,
        ..Default::default()
    };
    // Another start of this executable finds the window by this title.
    let title = HSTRING::from(instance::name());
    // SAFETY: the class name is a static NUL-terminated string and the procedure lives for
    // the whole process. A zero result leaves window creation below to fail.
    unsafe { RegisterClassW(&description) };
    // SAFETY: the class was registered above with this module; the window is never shown,
    // and a top-level window, unlike a message-only one, receives shell broadcasts.
    let Ok(window) = (unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class,
            &title,
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
    for message in [NOTIFY, taskbar_created, WM_CLIPBOARDUPDATE, OPEN, EVENTS] {
        // Explorer, the applications that copy and a second start of this executable run
        // at medium integrity; an elevated instance must accept their messages.
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
            events: RefCell::new(Vec::new()),
            wake: Box::new(wake),
        });
    });
    WINDOW.store(window.0 as isize, Ordering::Release);
    true
}

/// Asks for the application window from any thread, as a click on the icon does.
pub fn request_open() {
    post_message(OPEN);
}

/// Posts `message` to the tray window from any thread.
fn post_message(message: u32) {
    let window = WINDOW.load(Ordering::Acquire);
    if window != 0 {
        // SAFETY: posting to the tray window has no memory preconditions; a window that is
        // gone only fails the call.
        let _ =
            unsafe { PostMessageW(Some(HWND(window as *mut _)), message, WPARAM(0), LPARAM(0)) };
    }
}

/// Hands an event to the GUI from any thread.
pub(crate) fn post(event: DesktopEvent) {
    POSTED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(event);
    post_message(EVENTS);
}

/// Hands an event to the GUI from the GUI thread.
pub(crate) fn push(event: DesktopEvent) {
    with(|tray| {
        tray.events.borrow_mut().push(event);
        (tray.wake)();
    });
}

/// Events since the last call, in the order they happened on each thread: the GUI
/// thread's first, then those other threads posted.
pub fn take_events() -> Vec<DesktopEvent> {
    let mut events = with(|tray| tray.events.take()).unwrap_or_default();
    events.append(
        &mut POSTED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    events
}

/// The tray window, as other threads see it.
pub(crate) fn window_handle() -> Option<HWND> {
    let window = WINDOW.load(Ordering::Acquire);
    (window != 0).then_some(HWND(window as *mut _))
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

/// The hidden window of the calling thread's tray.
pub(crate) fn window() -> Option<HWND> {
    with(|tray| tray.window)
}

/// Asks the GUI for a frame, in which it also takes the events, from the GUI thread.
pub(crate) fn wake() {
    with(|tray| (tray.wake)());
}

/// Shows the icon with a tooltip and the labels of its "open", "screenshot", "note" and
/// "exit" menu items, or updates the text of the shown icon.
pub fn show(tip: &str, open: &str, screenshot: &str, note: &str, exit: &str) {
    with(|tray| {
        let labels = Labels {
            tip: tip.into(),
            open: open.into(),
            screenshot: screenshot.into(),
            note: note.into(),
            exit: exit.into(),
        };
        if tray.labels.replace(Some(labels)).is_some() {
            tray.notify(NIM_MODIFY);
        } else {
            tray.add();
        }
    });
}

/// Shows a balloon from the shown icon with a title and a text, marked as a warning when
/// `warning` is set. The balloon is silent and waits out the user's quiet time; clicking it
/// asks for the application window, like the icon.
pub fn balloon(title: &str, text: &str, warning: bool) {
    with(|tray| {
        if tray.labels.borrow().is_none() {
            return;
        }
        let mut data = tray.data();
        data.uFlags |= NIF_INFO;
        fill(&mut data.szInfoTitle, title);
        fill(&mut data.szInfo, text);
        data.dwInfoFlags =
            if warning { NIIF_WARNING } else { NIIF_USER } | NIIF_NOSOUND | NIIF_RESPECT_QUIET_TIME;
        // SAFETY: `data` is a fully initialized NOTIFYICONDATAW with its size set, naming
        // this tray's live window and icon.
        let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
    });
}

/// Copies `text` into a fixed text field, cut to leave the last unit as the terminating NUL.
fn fill(field: &mut [u16], text: &str) {
    let room = field.len() - 1;
    field.fill(0);
    for (slot, unit) in field[..room].iter_mut().zip(text.encode_utf16()) {
        *slot = unit;
    }
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

/// Identity of a shortcut among the tray window's shortcuts.
fn hotkey_id(shortcut: Shortcut) -> i32 {
    match shortcut {
        Shortcut::Screenshot => 1,
        Shortcut::Note => 2,
        Shortcut::Command(index) => COMMAND_HOTKEYS + index as i32,
    }
}

/// Registers a shortcut, replacing its previous keys, or with `None` removes it.
pub fn set_hotkey(shortcut: Shortcut, hotkey: Option<Hotkey>) -> Result<(), Reason> {
    let id = hotkey_id(shortcut);
    with(|tray| {
        // SAFETY: the shortcut belongs to this thread's own window; removing a shortcut that
        // is not registered only fails.
        let _ = unsafe { UnregisterHotKey(Some(tray.window), id) };
        let Some(hotkey) = hotkey else {
            return Ok(());
        };
        let mut modifiers = MOD_NOREPEAT;
        for (held, modifier) in [
            (hotkey.ctrl, MOD_CONTROL),
            (hotkey.alt, MOD_ALT),
            (hotkey.shift, MOD_SHIFT),
        ] {
            if held {
                modifiers |= modifier;
            }
        }
        // SAFETY: plain value arguments naming this thread's own window.
        unsafe { RegisterHotKey(Some(tray.window), id, modifiers, virtual_key(hotkey.key)) }
            .map_err(|error| {
                if error.code() == ERROR_HOTKEY_ALREADY_REGISTERED.to_hresult() {
                    Reason::HotkeyTaken
                } else {
                    Reason::CallFailed {
                        api: "RegisterHotKey",
                        code: error.code().0 as u32,
                    }
                }
            })
    })
    .unwrap_or(Err(Reason::CallFailed {
        api: "RegisterHotKey",
        code: ERROR_INVALID_WINDOW_HANDLE.0,
    }))
}

/// Virtual-key code of a shortcut key.
fn virtual_key(key: HotkeyKey) -> u32 {
    match key {
        // Letters and digits share their ASCII codes.
        HotkeyKey::Letter(c) | HotkeyKey::Digit(c) => c as u32,
        // VK_F1 is 0x70, and F1 to F24 follow it in order.
        HotkeyKey::Function(n) => 0x70 + u32::from(n) - 1,
    }
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
            fill(&mut data.szTip, &labels.tip);
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
}

/// Shows the icon's menu at a screen point and acts on the chosen item.
fn menu(window: HWND, x: i32, y: i32) {
    let Some(Some((open, screenshot, note, exit, main))) = with(|tray| {
        let labels = tray.labels.borrow();
        let labels = labels.as_ref()?;
        Some((
            HSTRING::from(&labels.open),
            HSTRING::from(&labels.screenshot),
            HSTRING::from(&labels.note),
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
        let _ = AppendMenuW(menu, MF_STRING, MENU_SCREENSHOT, &screenshot);
        let _ = AppendMenuW(menu, MF_STRING, MENU_NOTE, &note);
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
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
        MENU_OPEN => push(DesktopEvent::Open),
        // The screen is captured once the menu has left it.
        // SAFETY: the timer belongs to this thread's own window.
        MENU_SCREENSHOT => unsafe {
            SetTimer(Some(window), SCREENSHOT_TIMER, MENU_FADE_MS, None);
        },
        MENU_NOTE => push(DesktopEvent::Note),
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
                NIN_SELECT | KEY_SELECT | NIN_BALLOONUSERCLICK => push(DesktopEvent::Open),
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
        OPEN => {
            push(DesktopEvent::Open);
            LRESULT(0)
        }
        EVENTS => {
            wake();
            LRESULT(0)
        }
        WM_HOTKEY if wparam.0 == hotkey_id(Shortcut::Screenshot) as usize => {
            push(DesktopEvent::Screenshot);
            LRESULT(0)
        }
        WM_HOTKEY if wparam.0 == hotkey_id(Shortcut::Note) as usize => {
            push(DesktopEvent::Note);
            LRESULT(0)
        }
        WM_HOTKEY if wparam.0 >= COMMAND_HOTKEYS as usize => {
            push(DesktopEvent::Command(wparam.0 - COMMAND_HOTKEYS as usize));
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == SCREENSHOT_TIMER => {
            // SAFETY: stops this thread's own one-shot timer.
            let _ = unsafe { KillTimer(Some(window), SCREENSHOT_TIMER) };
            push(DesktopEvent::Screenshot);
            LRESULT(0)
        }
        WM_CLIPBOARDUPDATE => {
            clipboard::changed();
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
