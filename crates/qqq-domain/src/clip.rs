//! Screenshots, clipboard contents and the keyboard shortcuts that start a screenshot or
//! show the note window.

use std::{fmt, sync::Arc};

/// An image with 8-bit RGBA pixels, row by row from the top.
#[derive(Clone, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    /// `width × height × 4` bytes.
    pub rgba: Vec<u8>,
}

impl fmt::Debug for Picture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Picture({} × {})", self.width, self.height)
    }
}

impl Picture {
    /// Bytes the pixels take.
    pub fn bytes(&self) -> usize {
        self.rgba.len()
    }
}

/// Content of the clipboard in a form that can be put back on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Clip {
    Text(String),
    Picture(Arc<Picture>),
    /// Paths of copied files and folders.
    Files(Vec<String>),
}

impl Clip {
    /// Approximate bytes the content takes in memory.
    pub fn bytes(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Picture(picture) => picture.bytes(),
            Self::Files(paths) => paths.iter().map(String::len).sum(),
        }
    }
}

/// How a screen capture ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureEnd {
    /// The pixels of the region the user selected.
    Taken(Picture),
    /// The user left without selecting a region.
    Cancelled,
}

/// Key of a keyboard shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyKey {
    /// `A` to `Z`.
    Letter(char),
    /// `0` to `9`.
    Digit(char),
    /// F1 to F24, by number.
    Function(u8),
}

/// What a keyboard shortcut registered with the system does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    /// Starts a screenshot.
    Screenshot,
    /// Shows or hides the note window.
    Note,
}

impl Shortcut {
    pub const ALL: [Self; 2] = [Self::Screenshot, Self::Note];
}

/// A keyboard shortcut: modifier keys held with one key, written like `Ctrl+Alt+A`.
/// Letters and digits need Ctrl or Alt, so the shortcut never takes over typing; function
/// keys work alone too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: HotkeyKey,
}

impl Hotkey {
    /// Ctrl+Alt+A, the default screenshot shortcut.
    pub const SCREENSHOT: Self = Self {
        ctrl: true,
        alt: true,
        shift: false,
        key: HotkeyKey::Letter('A'),
    };
    /// Ctrl+Alt+N, the default note shortcut.
    pub const NOTE: Self = Self {
        ctrl: true,
        alt: true,
        shift: false,
        key: HotkeyKey::Letter('N'),
    };

    /// Whether the key exists and the modifiers suit it.
    pub fn is_valid(&self) -> bool {
        match self.key {
            HotkeyKey::Letter(c) => c.is_ascii_uppercase() && (self.ctrl || self.alt),
            HotkeyKey::Digit(c) => c.is_ascii_digit() && (self.ctrl || self.alt),
            HotkeyKey::Function(n) => (1..=24).contains(&n),
        }
    }

    /// Reads the written form, such as `Ctrl+Shift+S` or `F1`; names are case-insensitive.
    pub fn parse(text: &str) -> Option<Self> {
        let mut hotkey = Self {
            ctrl: false,
            alt: false,
            shift: false,
            key: HotkeyKey::Function(0),
        };
        let mut parts = text.split('+').map(str::trim).peekable();
        while let Some(part) = parts.next() {
            let upper = part.to_ascii_uppercase();
            if parts.peek().is_some() {
                let flag = match upper.as_str() {
                    "CTRL" => &mut hotkey.ctrl,
                    "ALT" => &mut hotkey.alt,
                    "SHIFT" => &mut hotkey.shift,
                    _ => return None,
                };
                if std::mem::replace(flag, true) {
                    return None;
                }
                continue;
            }
            let mut chars = upper.chars();
            hotkey.key = match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_uppercase() => HotkeyKey::Letter(c),
                (Some(c), None) if c.is_ascii_digit() => HotkeyKey::Digit(c),
                (Some('F'), Some(_)) => HotkeyKey::Function(upper[1..].parse().ok()?),
                _ => return None,
            };
        }
        hotkey.is_valid().then_some(hotkey)
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (held, name) in [
            (self.ctrl, "Ctrl+"),
            (self.alt, "Alt+"),
            (self.shift, "Shift+"),
        ] {
            if held {
                f.write_str(name)?;
            }
        }
        match self.key {
            HotkeyKey::Letter(c) | HotkeyKey::Digit(c) => write!(f, "{c}"),
            HotkeyKey::Function(n) => write!(f, "F{n}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkeys_read_back_their_written_form() {
        for text in [
            "Ctrl+Alt+A",
            "Ctrl+Shift+S",
            "Alt+1",
            "F1",
            "Shift+F12",
            "Ctrl+F24",
        ] {
            assert_eq!(Hotkey::parse(text).unwrap().to_string(), text);
        }
        assert_eq!(Hotkey::parse(" alt + ctrl + a "), Some(Hotkey::SCREENSHOT));
        assert_eq!(Hotkey::SCREENSHOT.to_string(), "Ctrl+Alt+A");
        assert_eq!(Hotkey::NOTE.to_string(), "Ctrl+Alt+N");
    }

    #[test]
    fn hotkeys_keep_typing_free() {
        for text in [
            "A",
            "Shift+A",
            "1",
            "Ctrl+Ctrl+A",
            "Ctrl+",
            "Ctrl+Win+A",
            "F0",
            "F25",
            "Ctrl+AB",
            "",
        ] {
            assert_eq!(Hotkey::parse(text), None, "{text}");
        }
    }
}
