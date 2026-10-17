//! Full-screen applications in the foreground. The docked bar lets clicks through to one on
//! its monitor; following foreground changes wakes the GUI as soon as one comes forward or
//! leaves.

use crate::{
    native::{point, text},
    tray,
};
use std::{cell::Cell, mem::size_of};
use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONULL, MONITORINFO, MonitorFromPoint, MonitorFromWindow,
    },
    System::Threading::GetCurrentProcessId,
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
        WindowsAndMessaging::{
            EVENT_SYSTEM_FOREGROUND, GWL_STYLE, GetClassNameW, GetForegroundWindow, GetWindowLongW,
            GetWindowRect, GetWindowThreadProcessId, IsZoomed, WINEVENT_OUTOFCONTEXT,
            WINEVENT_SKIPOWNPROCESS, WS_CAPTION,
        },
    },
};

thread_local! {
    /// Hook that reports foreground changes, present while they are followed.
    static HOOK: Cell<Option<HWINEVENTHOOK>> = const { Cell::new(None) };
}

/// Whether the foreground window belongs to another process and covers the whole monitor
/// that holds a physical-pixel point, as a full-screen game or video does. The desktop and
/// maximized windows with a title bar, which reach every monitor edge while the taskbar
/// hides itself, count as ordinary windows.
pub fn at(at: [f32; 2]) -> bool {
    // SAFETY: plain call; a null window is handled below.
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() || own(window) || desktop(window) || titled_maximized(window) {
        return false;
    }
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let mut rect = RECT::default();
    // SAFETY: the point and window are passed by value and only read; `info` is a writable
    // MONITORINFO with its size set and `rect` a writable local.
    let measured = unsafe {
        let monitor = MonitorFromPoint(point(at), MONITOR_DEFAULTTONULL);
        !monitor.is_invalid()
            && MonitorFromWindow(window, MONITOR_DEFAULTTONULL) == monitor
            && GetMonitorInfoW(monitor, &mut info).as_bool()
            && GetWindowRect(window, &mut rect).is_ok()
    };
    measured && covers(rect, info.rcMonitor)
}

/// Starts or stops waking the GUI whenever a window of another process comes to the
/// foreground. Only the GUI thread that attached the tray is woken.
pub fn watch(on: bool) {
    HOOK.with(|hook| match (hook.get(), on) {
        (None, true) => {
            // SAFETY: an out-of-context hook needs no module; the system calls `changed`
            // through this thread's message loop until the hook is removed.
            let handle = unsafe {
                SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    None,
                    Some(changed),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                )
            };
            if !handle.is_invalid() {
                hook.set(Some(handle));
            }
        }
        (Some(handle), false) => {
            // SAFETY: the hook was set on this thread and is removed once.
            let _ = unsafe { UnhookWinEvent(handle) };
            hook.set(None);
        }
        _ => {}
    });
}

unsafe extern "system" fn changed(
    _: HWINEVENTHOOK,
    _: u32,
    _: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    tray::wake();
}

fn own(window: HWND) -> bool {
    let mut pid = 0;
    // SAFETY: the window handle is only read, and `pid` is a writable local.
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    // SAFETY: plain call.
    pid == 0 || pid == unsafe { GetCurrentProcessId() }
}

/// Whether the window is the desktop, which spans the monitors behind every other window.
fn desktop(window: HWND) -> bool {
    let mut class = [0_u16; 16];
    // SAFETY: the buffer is a writable local whose length bounds the copied name.
    unsafe { GetClassNameW(window, &mut class) };
    matches!(text(&class).as_str(), "Progman" | "WorkerW")
}

fn titled_maximized(window: HWND) -> bool {
    // SAFETY: both calls only read the window's state.
    unsafe {
        IsZoomed(window).as_bool()
            && GetWindowLongW(window, GWL_STYLE) as u32 & WS_CAPTION.0 == WS_CAPTION.0
    }
}

fn covers(window: RECT, monitor: RECT) -> bool {
    window.left <= monitor.left
        && window.top <= monitor.top
        && window.right >= monitor.right
        && window.bottom >= monitor.bottom
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn a_window_is_full_screen_when_it_covers_the_whole_monitor() {
        let monitor = rect(1920, 0, 3840, 1080);
        assert!(covers(rect(1920, 0, 3840, 1080), monitor));
        assert!(covers(rect(1900, -20, 3860, 1100), monitor));
        // A maximized window stops at the taskbar.
        assert!(!covers(rect(1912, -8, 3848, 1040), monitor));
        assert!(!covers(rect(0, 0, 1920, 1080), monitor));
    }
}
