//! What the GUI asks of the host platform, grouped by capability, and the events the
//! desktop reports back. The starting program fills in the functions; points are in
//! physical pixels of the virtual screen. The functions act on the GUI thread.

use qqq_domain::{Clip, CommandSound, DesktopEvent, Hotkey, Reason, ShellSession, Shortcut};

/// Rectangle edges as `[left, top, right, bottom]`.
pub type Edges = [f32; 4];

/// Starts a PowerShell session with a script, a window title, whether it shows a window, and
/// a working directory, empty for the user's profile folder.
pub type StartPowerShell = fn(&str, &str, bool, &str) -> Result<Box<dyn ShellSession>, Reason>;

/// The system's interface font for a language tag, as font file bytes and the index of the
/// face; `None` where the embedded font serves.
pub type UiFont = fn(&str) -> Option<(Vec<u8>, u32)>;

/// The platform's services, and the events it reports.
#[derive(Clone, Copy)]
pub struct Desktop {
    pub display: Display,
    pub shell: Shell,
    pub input: Input,
    pub system: System,
    /// Events since the last call, in the order they happened: requests from the
    /// notification-area icon and the shortcuts, region selections that ended, clipboard
    /// changes, and files or addresses that did not open.
    pub take_events: fn() -> Vec<DesktopEvent>,
}

/// Monitors, the pointer and the windows of other applications, for docking.
#[derive(Clone, Copy)]
pub struct Display {
    /// Bounds and work area of the monitor nearest to a point.
    pub monitor_at: fn([f32; 2]) -> Option<(Edges, Edges)>,
    /// Whether a point lies on any monitor.
    pub covers: fn([f32; 2]) -> bool,
    /// Whether a mouse button is held, including while the system moves the window.
    pub button_held: fn() -> bool,
    /// Whether a full-screen window of another process is in the foreground on the monitor
    /// that holds a point.
    pub fullscreen_at: fn([f32; 2]) -> bool,
    /// Starts or stops waking the GUI whenever another application comes to the foreground.
    pub watch_foreground: fn(bool),
}

/// The notification-area icon, the taskbar button and the drawing of the application
/// window.
#[derive(Clone, Copy)]
pub struct Shell {
    /// Shows the notification-area icon with a tooltip and the labels of its "open",
    /// "screenshot", "note" and "exit" menu items, or updates the text of the shown icon.
    pub show_icon: fn(&str, &str, &str, &str, &str),
    /// Removes the notification-area icon.
    pub hide_icon: fn(),
    /// Shows a silent balloon from the notification-area icon with a title and a text,
    /// marked as a warning when the flag is set; nothing while the icon is hidden.
    pub balloon: fn(&str, &str, bool),
    /// Shows or removes the taskbar button of the visible window; `false` while the window
    /// has not appeared yet.
    pub taskbar_button: fn(bool) -> bool,
    /// Cloaks or reveals the application window: the compositor draws nothing of a cloaked
    /// window, whose visibility stays as it is.
    pub cloak: fn(bool),
}

/// Shortcuts, region selection and the clipboard.
#[derive(Clone, Copy)]
pub struct Input {
    /// Registers a shortcut in place of its previous keys, or with `None` removes it.
    pub set_hotkey: fn(Shortcut, Option<Hotkey>) -> Result<(), Reason>,
    /// Freezes the screen for the user to select a region, showing this hint; the outcome
    /// arrives as an event.
    pub start_capture: fn(&str) -> Result<(), Reason>,
    /// Starts or stops following the clipboard, whose changes arrive as events; starting
    /// counts the current content as a change. Content the application puts there itself
    /// arrives as no event.
    pub watch_clipboard: fn(bool) -> Result<(), Reason>,
    /// Puts content on the clipboard.
    pub set_clipboard: fn(&Clip) -> Result<(), Reason>,
}

/// Other services of the system.
#[derive(Clone, Copy)]
pub struct System {
    /// Whether the program starts when the user signs in.
    pub autostart: fn() -> bool,
    /// Switches starting at sign-in on or off.
    pub set_autostart: fn(bool) -> Result<(), Reason>,
    /// Shows a system notification with a title and a text.
    pub notify: fn(&str, &str) -> Result<(), Reason>,
    /// Gets the system ready to show the application's notifications, in the background.
    pub prepare_notifications: fn(),
    /// Starts the application again with administrator rights and these arguments.
    pub restart_elevated: fn(&str) -> Result<(), Reason>,
    /// Starts a PowerShell session and returns it to follow and terminate. A shown window
    /// stays open after the script ends; a hidden session ends with it.
    pub start_powershell: StartPowerShell,
    /// Plays a short sound without waiting for it.
    pub play_sound: fn(CommandSound),
    /// Opens a file with its default program, or a folder, in the background; a failure
    /// arrives as an event.
    pub open_path: fn(&std::path::Path),
    /// Opens a web address in the default browser, like `open_path`.
    pub open_url: fn(&str),
    /// Local date and time as year, month, day, hour, minute, second and millisecond.
    pub local_time: fn(std::time::SystemTime) -> Option<[u16; 7]>,
    /// The system's interface font for a language.
    pub ui_font: UiFont,
}
