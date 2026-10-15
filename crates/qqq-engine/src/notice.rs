use qqq_domain::Reason;

/// Outcome of a command or a configuration operation that the user should see. Notices
/// are language-neutral; the GUI phrases them in the interface language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// The monitor service has stopped.
    Stopped,
    /// Too many commands are waiting for the engine.
    Busy,
    /// A history query asked for too many series or an unknown range.
    InvalidQuery,
    /// The sampling interval is not 0.5, 1 or 2 seconds.
    InvalidSampling,
    /// The history range is not 60, 300 or 900 seconds.
    InvalidHistory,
    InvalidWindowSize,
    InvalidWindowPosition,
    /// Some preferences held invalid values and use their defaults.
    InvalidValue,
    /// Some preferences held values of the wrong type and use their defaults.
    InvalidType,
    /// The configuration file is not valid TOML; defaults are in use and the file stays
    /// until the user saves settings.
    CorruptFile,
    /// The configuration file could not be read, with the system's description.
    Unreadable(String),
    /// A damaged configuration file is kept until the user saves from the settings page.
    CorruptKept,
    /// The configuration path has no parent directory.
    InvalidDirectory,
    /// An operating system or library error, with its own description.
    System(String),
    /// A process action failed.
    Process(Reason),
    /// A remote host entry lacks an address or user, uses an invalid port, or repeats the
    /// identity of another entry.
    InvalidRemoteHost,
    /// The password or passphrase of a remote host could not be saved or removed.
    SecretUnsaved(Reason),
    /// Starting at sign-in could not be switched on or off.
    AutostartUnchanged(Reason),
    /// An alert rule lacks a device or metric, has an invalid threshold or hold time, or
    /// repeats the identity of another rule.
    InvalidAlertRule,
    /// A notification could not be shown.
    NotificationFailed(Reason),
    /// The application could not start again with administrator rights.
    ElevationFailed(Reason),
    /// The screenshot shortcut is not a valid key combination.
    InvalidHotkey,
    /// The number of screenshots or clipboard entries to keep is not one of the choices.
    InvalidRecordLimit,
    /// The screenshot shortcut could not be registered.
    HotkeyUnavailable(Reason),
    /// The screen could not be captured.
    ScreenshotFailed(Reason),
    /// A screenshot file could not be written, read or removed, with the system's
    /// description.
    ScreenshotFile(String),
    /// Content could not be put on the clipboard.
    ClipboardFailed(Reason),
    /// A file or folder could not be opened.
    OpenFailed(Reason),
}

/// State of the settings: how they were loaded, or what became of the latest save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveStatus {
    Loaded,
    /// Loaded with defaults for the parts the notice describes.
    Recovered(Notice),
    Saving,
    Saved,
    /// Applied to this session but not written.
    Unsaved(Notice),
}
