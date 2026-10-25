//! What the desktop reports to the GUI: requests from the notification-area icon, the
//! shortcuts and other starts of the program, and the outcomes of work done for the GUI.

use crate::{CaptureEnd, Clip, Reason};

/// Something the desktop reports to the GUI, in the order it happened.
#[derive(Debug)]
pub enum DesktopEvent {
    /// The user asked for the application window: from the notification-area icon, its menu
    /// or a balloon, a notification, or another start of the program.
    Open,
    /// The screenshot shortcut or the icon's menu asked for a screenshot.
    Screenshot,
    /// The note shortcut or the icon's menu asked for the note window.
    Note,
    /// The shortcut registered under this number for a quick command was pressed.
    Command(usize),
    /// A region selection ended.
    Capture(CaptureEnd),
    /// Content that another application put on the clipboard, while it is followed.
    Clipboard(Clip),
    /// A file, folder or web address did not open.
    OpenFailed { target: String, reason: Reason },
}
