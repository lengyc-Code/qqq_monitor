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
