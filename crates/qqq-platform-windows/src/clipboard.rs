//! The clipboard: following its changes, reading text, pictures and file lists from it, and
//! putting them back.
//!
//! The hidden tray window listens to clipboard changes and owns what this module puts on the
//! clipboard, so these functions act on the GUI thread that attached the tray.

use crate::tray;
use qqq_domain::{Clip, Picture, Reason};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread,
    time::Duration,
};
use windows::{
    Win32::{
        Foundation::{ERROR_INVALID_WINDOW_HANDLE, GlobalFree, HANDLE, HGLOBAL, HWND},
        System::{
            DataExchange::{
                AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData,
                IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
                RemoveClipboardFormatListener, SetClipboardData,
            },
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
            Ole::{CF_DIB, CF_DIBV5, CF_HDROP, CF_UNICODETEXT, DROPEFFECT_COPY},
        },
        UI::Shell::{DragQueryFileW, HDROP},
    },
    core::{PCWSTR, w},
};

/// Largest content read from the clipboard, in bytes; a 7680 × 4320 picture fits.
pub const MAX_BYTES: usize = 128 * 1024 * 1024;

/// Wait before reading the latest change again while another application keeps the
/// clipboard open, in milliseconds; each further attempt waits twice as long.
const RETRY_MS: u32 = 25;
/// Further attempts to read the latest change, about 6 seconds altogether.
const RETRIES: u32 = 8;

/// The tray window listens to clipboard changes.
static WATCHING: AtomicBool = AtomicBool::new(false);
/// The clipboard changed since the last [`take`].
static CHANGED: AtomicBool = AtomicBool::new(false);
/// Attempts that found the latest change's clipboard open by another application.
static BUSY: AtomicU32 = AtomicU32::new(0);

/// Called by the tray window when the clipboard changes.
pub(crate) fn changed() {
    if WATCHING.load(Ordering::Relaxed) {
        BUSY.store(0, Ordering::Relaxed);
        tray::clipboard_timer(None);
        CHANGED.store(true, Ordering::Relaxed);
        tray::wake();
    }
}

/// Called by the tray window when the wait after a busy clipboard has passed.
pub(crate) fn retry() {
    if WATCHING.load(Ordering::Relaxed) {
        CHANGED.store(true, Ordering::Relaxed);
        tray::wake();
    }
}

/// Starts or stops following the clipboard. Starting counts the content already on the
/// clipboard as the first change.
pub fn watch(on: bool) -> Result<(), Reason> {
    if WATCHING.load(Ordering::Relaxed) == on {
        return Ok(());
    }
    let window = tray::window().ok_or(Reason::CallFailed {
        api: "AddClipboardFormatListener",
        code: ERROR_INVALID_WINDOW_HANDLE.0,
    })?;
    // SAFETY: the window belongs to this thread and outlives its listener registration.
    unsafe {
        if on {
            AddClipboardFormatListener(window)
                .map_err(|error| failed("AddClipboardFormatListener", error))?;
        } else {
            let _ = RemoveClipboardFormatListener(window);
        }
    }
    WATCHING.store(on, Ordering::Relaxed);
    CHANGED.store(on, Ordering::Relaxed);
    BUSY.store(0, Ordering::Relaxed);
    tray::clipboard_timer(None);
    Ok(())
}

/// Content put on the clipboard since the last call: its files, otherwise its text,
/// otherwise its picture. `None` when nothing changed, when the content is none of these
/// or larger than [`MAX_BYTES`], or when its source asked clipboard monitors to skip it,
/// as password managers do. While another application keeps the clipboard open, the
/// change is read again after a growing wait.
pub fn take() -> Option<Clip> {
    if !CHANGED.swap(false, Ordering::Relaxed) {
        return None;
    }
    let Some(_open) = Open::once(tray::window()?) else {
        let attempt = BUSY.fetch_add(1, Ordering::Relaxed);
        if attempt < RETRIES {
            tray::clipboard_timer(Some(RETRY_MS << attempt));
        } else {
            tracing::warn!("剪贴板持续被其他程序占用，跳过本次变化");
        }
        return None;
    };
    BUSY.store(0, Ordering::Relaxed);
    if private() {
        return None;
    }
    files()
        .or_else(text)
        .or_else(|| picture(CF_DIBV5.0).or_else(|| picture(CF_DIB.0)))
}

/// Puts content on the clipboard in place of what it holds.
pub fn set(clip: &Clip) -> Result<(), Reason> {
    let window = tray::window().ok_or(Reason::CallFailed {
        api: "OpenClipboard",
        code: ERROR_INVALID_WINDOW_HANDLE.0,
    })?;
    let _open = Open::new(window)?;
    // SAFETY: the clipboard is open by this thread; emptying it makes the window its owner.
    unsafe { EmptyClipboard() }.map_err(|error| failed("EmptyClipboard", error))?;
    match clip {
        Clip::Text(text) => {
            let units: Vec<u16> = text.encode_utf16().chain([0]).collect();
            let bytes: Vec<u8> = units.iter().flat_map(|u| u.to_le_bytes()).collect();
            put(CF_UNICODETEXT.0, &bytes)
        }
        Clip::Picture(picture) => put(CF_DIB.0, &dib(picture)),
        Clip::Files(paths) => {
            put(CF_HDROP.0, &drop_files(paths))?;
            // SAFETY: the format name is a static NUL-terminated string.
            let effect = unsafe { RegisterClipboardFormatW(w!("Preferred DropEffect")) };
            if effect != 0 {
                put(effect as u16, &DROPEFFECT_COPY.0.to_le_bytes())?;
            }
            Ok(())
        }
    }
}

/// The clipboard, open for the calling thread until dropped.
struct Open;

impl Open {
    /// Opens the clipboard for `window`; `None` while another application has it open.
    fn once(window: HWND) -> Option<Self> {
        // SAFETY: the window belongs to this thread; a successful open is closed on drop.
        unsafe { OpenClipboard(Some(window)) }
            .is_ok()
            .then_some(Self)
    }

    /// Opens the clipboard for `window`, waiting briefly while another application has it
    /// open.
    fn new(window: HWND) -> Result<Self, Reason> {
        for _ in 0..10 {
            if let Some(open) = Self::once(window) {
                return Ok(open);
            }
            thread::sleep(Duration::from_millis(10));
        }
        Err(Reason::ClipboardBusy)
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: closes the clipboard this thread opened.
        let _ = unsafe { CloseClipboard() };
    }
}

fn failed(api: &'static str, error: windows::core::Error) -> Reason {
    Reason::CallFailed {
        api,
        code: error.code().0 as u32,
    }
}

/// Clipboard format registered under `name`, or 0.
fn format(name: PCWSTR) -> u32 {
    // SAFETY: the name is a static NUL-terminated string.
    unsafe { RegisterClipboardFormatW(name) }
}

fn available(format: u32) -> bool {
    // SAFETY: plain value argument; the clipboard is open.
    format != 0 && unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

/// Whether the content's source asked clipboard monitors and clipboard history to skip it.
fn private() -> bool {
    if available(format(w!("ExcludeClipboardContentFromMonitorProcessing")))
        || available(format(w!("Clipboard Viewer Ignore")))
    {
        return true;
    }
    let history = format(w!("CanIncludeInClipboardHistory"));
    available(history)
        && data(history).is_some_and(|bytes| bytes.get(..4) == Some(&[0, 0, 0, 0][..]))
}

/// Copy of the clipboard data in `format`, at most [`MAX_BYTES`] long. The clipboard must
/// be open.
fn data(format: u32) -> Option<Vec<u8>> {
    // SAFETY: the clipboard is open; the handle stays valid until it closes, and the memory
    // is read only between the lock and the unlock, within the size the system reports.
    unsafe {
        let global = HGLOBAL(GetClipboardData(format).ok()?.0);
        let size = GlobalSize(global);
        if size == 0 || size > MAX_BYTES {
            return None;
        }
        let pointer = GlobalLock(global);
        if pointer.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(pointer.cast::<u8>(), size).to_vec();
        let _ = GlobalUnlock(global);
        Some(bytes)
    }
}

fn text() -> Option<Clip> {
    let bytes = data(u32::from(CF_UNICODETEXT.0))?;
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .take_while(|unit| *unit != 0)
        .collect();
    (!units.is_empty()).then(|| Clip::Text(String::from_utf16_lossy(&units)))
}

fn files() -> Option<Clip> {
    // SAFETY: the clipboard is open, so the handle stays valid while the names are read
    // into buffers of the length the first query reports.
    let paths: Vec<String> = unsafe {
        let list = HDROP(GetClipboardData(u32::from(CF_HDROP.0)).ok()?.0);
        let count = DragQueryFileW(list, u32::MAX, None);
        (0..count)
            .filter_map(|index| {
                let length = DragQueryFileW(list, index, None) as usize;
                let mut buffer = vec![0_u16; length + 1];
                let copied = DragQueryFileW(list, index, Some(&mut buffer)) as usize;
                (copied > 0).then(|| String::from_utf16_lossy(&buffer[..copied]))
            })
            .collect()
    };
    (!paths.is_empty()).then_some(Clip::Files(paths))
}

fn picture(format: u16) -> Option<Clip> {
    let picture = decode_dib(&data(u32::from(format))?)?;
    Some(Clip::Picture(Arc::new(picture)))
}

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// `BI_RGB` and `BI_BITFIELDS` compression of a device-independent bitmap.
const BI_RGB: u32 = 0;
const BI_BITFIELDS: u32 = 3;

/// Reads a packed device-independent bitmap, as in `CF_DIB` or `CF_DIBV5`: an info header,
/// optional color masks and color table, then the rows. Supports uncompressed 24-bit and
/// 32-bit pixels with the usual byte order. A 32-bit bitmap whose alpha bytes are all zero
/// is opaque.
pub(crate) fn decode_dib(bytes: &[u8]) -> Option<Picture> {
    let header = read_u32(bytes, 0)? as usize;
    let width = read_u32(bytes, 4)? as i32;
    let height = read_u32(bytes, 8)? as i32;
    let depth = read_u16(bytes, 14)?;
    let compression = read_u32(bytes, 16)?;
    let colors = read_u32(bytes, 32)? as usize;
    if header < 40 || width <= 0 || height == 0 || height == i32::MIN {
        return None;
    }
    let (width, rows) = (width as usize, height.unsigned_abs() as usize);
    let masks = match (compression, depth) {
        (BI_RGB, 24 | 32) => None,
        // A plain info header is followed by the masks; larger headers hold them at the
        // same place.
        (BI_BITFIELDS, 32) => Some([
            read_u32(bytes, 40)?,
            read_u32(bytes, 44)?,
            read_u32(bytes, 48)?,
        ]),
        _ => return None,
    };
    if masks.is_some_and(|m| m != [0x00FF_0000, 0x0000_FF00, 0x0000_00FF]) {
        return None;
    }
    let extra = if masks.is_some() && header == 40 {
        12
    } else {
        0
    };
    let start = header + extra + colors.min(256) * 4;
    let pixel = usize::from(depth / 8);
    let stride = (width * usize::from(depth)).div_ceil(32) * 4;
    let size = width.checked_mul(rows)?.checked_mul(4)?;
    if size > MAX_BYTES || bytes.len() < start + stride.checked_mul(rows)? {
        return None;
    }
    let mut rgba = Vec::with_capacity(size);
    let mut alpha = false;
    for y in 0..rows {
        // Positive heights store the bottom row first.
        let row = if height > 0 { rows - 1 - y } else { y };
        let line = &bytes[start + row * stride..][..width * pixel];
        for source in line.chunks_exact(pixel) {
            let a = if pixel == 4 { source[3] } else { 255 };
            alpha |= a != 0;
            rgba.extend_from_slice(&[source[2], source[1], source[0], a]);
        }
    }
    if !alpha {
        rgba.iter_mut().skip(3).step_by(4).for_each(|a| *a = 255);
    }
    Some(Picture {
        width: width as u32,
        height: rows as u32,
        rgba,
    })
}

/// A packed 32-bit device-independent bitmap of the picture, bottom row first, as most
/// applications expect from `CF_DIB`.
pub(crate) fn dib(picture: &Picture) -> Vec<u8> {
    let (width, height) = (picture.width as usize, picture.height as usize);
    let mut bytes = Vec::with_capacity(40 + width * height * 4);
    for value in [40_u32, picture.width, picture.height] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    for value in [BI_RGB, (width * height * 4) as u32, 0, 0, 0, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for row in picture.rgba.chunks_exact(width * 4).rev() {
        for p in row.as_chunks::<4>().0 {
            bytes.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    bytes
}

/// A `DROPFILES` structure followed by the NUL-terminated wide paths and a closing NUL.
fn drop_files(paths: &[String]) -> Vec<u8> {
    // pFiles, pt.x, pt.y, fNC, fWide.
    let mut bytes: Vec<u8> = [20_u32, 0, 0, 0, 1]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    for path in paths {
        bytes.extend(path.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
    }
    bytes.extend_from_slice(&[0, 0]);
    bytes
}

/// Puts a copy of `bytes` on the open clipboard in `format`.
fn put(format: u16, bytes: &[u8]) -> Result<(), Reason> {
    // SAFETY: the block is allocated with the length of `bytes` and written only while
    // locked. Once SetClipboardData succeeds the system owns it; otherwise it is freed here.
    unsafe {
        let global =
            GlobalAlloc(GMEM_MOVEABLE, bytes.len()).map_err(|e| failed("GlobalAlloc", e))?;
        let pointer = GlobalLock(global);
        if pointer.is_null() {
            let _ = GlobalFree(Some(global));
            return Err(Reason::CallFailed {
                api: "GlobalLock",
                code: 0,
            });
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast::<u8>(), bytes.len());
        let _ = GlobalUnlock(global);
        SetClipboardData(u32::from(format), Some(HANDLE(global.0))).map_err(|error| {
            let _ = GlobalFree(Some(global));
            failed("SetClipboardData", error)
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_survive_a_device_independent_bitmap() {
        let picture = Picture {
            width: 3,
            height: 2,
            rgba: (0..24).map(|v| v as u8 * 10).collect(),
        };
        assert_eq!(decode_dib(&dib(&picture)), Some(picture));
    }

    #[test]
    fn bitmaps_read_with_padding_masks_and_either_row_order() {
        // 2 × 2, 24-bit, bottom row first: rows are padded to 8 bytes.
        let mut bytes: Vec<u8> = [40_u32, 2, 2]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&24_u16.to_le_bytes());
        bytes.extend([0_u32; 6].iter().flat_map(|v| v.to_le_bytes()));
        bytes.extend_from_slice(&[1, 2, 3, 4, 5, 6, 0, 0, 7, 8, 9, 10, 11, 12, 0, 0]);
        let picture = decode_dib(&bytes).unwrap();
        assert_eq!(
            picture.rgba,
            [9, 8, 7, 255, 12, 11, 10, 255, 3, 2, 1, 255, 6, 5, 4, 255]
        );
        // 1 × 2, 32-bit with masks after the header, top row first, alpha all zero.
        let mut bytes: Vec<u8> = [40_u32, 1, (-2_i32) as u32]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&32_u16.to_le_bytes());
        bytes.extend(
            [BI_BITFIELDS, 0, 0, 0, 0, 0]
                .iter()
                .flat_map(|v| v.to_le_bytes()),
        );
        bytes.extend(
            [0x00FF_0000_u32, 0xFF00, 0xFF]
                .iter()
                .flat_map(|v| v.to_le_bytes()),
        );
        bytes.extend_from_slice(&[1, 2, 3, 0, 4, 5, 6, 0]);
        assert_eq!(
            decode_dib(&bytes).unwrap().rgba,
            [3, 2, 1, 255, 6, 5, 4, 255]
        );
    }

    #[test]
    fn unsupported_or_short_bitmaps_are_skipped() {
        let mut bytes: Vec<u8> = [40_u32, 4, 4]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&8_u16.to_le_bytes());
        bytes.extend([0_u32; 6].iter().flat_map(|v| v.to_le_bytes()));
        bytes.extend([0_u8; 64]);
        assert_eq!(decode_dib(&bytes), None);
        let mut short = dib(&Picture {
            width: 2,
            height: 2,
            rgba: vec![0; 16],
        });
        short.pop();
        assert_eq!(decode_dib(&short), None);
    }

    #[test]
    fn file_lists_are_wide_and_double_terminated() {
        let bytes = drop_files(&["C:\\a".into(), "D:\\é".into()]);
        assert_eq!(&bytes[..4], &20_u32.to_le_bytes());
        assert_eq!(&bytes[16..20], &1_u32.to_le_bytes());
        let units: Vec<u16> = bytes[20..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        let expected: Vec<u16> = "C:\\a\0D:\\é\0\0".encode_utf16().collect();
        assert_eq!(units, expected);
    }
}
