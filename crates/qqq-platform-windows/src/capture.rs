//! Screen capture with region selection. The whole virtual screen is copied once and shown,
//! frozen and dimmed, in a window that covers every monitor; the user drags out the region
//! to keep, or clicks to keep the monitor under the pointer.
//!
//! The window belongs to the GUI thread, whose message loop delivers its input and painting
//! as it does for the tray window. All functions act on the calling thread's state, and the
//! window works in physical pixels of the virtual screen.

use crate::tray;
use qqq_domain::{CaptureEnd, DesktopEvent, Picture, Reason};
use std::{cell::RefCell, mem::size_of};
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            AC_SRC_OVER, AlphaBlend, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
            BeginPaint, BitBlt, CAPTUREBLT, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS,
            CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET,
            DIB_RGB_COLORS, DRAW_TEXT_FORMAT, DT_CALCRECT, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE,
            DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, EndPaint, FW_NORMAL, FillRect, GdiFlush,
            GetDC, GetMonitorInfoW, HBITMAP, HDC, HFONT, HGDIOBJ, InvalidateRect,
            MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, OUT_DEFAULT_PRECIS,
            PAINTSTRUCT, ReleaseDC, SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            HiDpi::{
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
                SetThreadDpiAwarenessContext,
            },
            Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, GetSystemMetrics,
                IDC_CROSS, LoadCursorW, RegisterClassW, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
                SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOW, SetForegroundWindow, ShowWindow,
                WA_INACTIVE, WM_ACTIVATE, WM_DPICHANGED, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN,
                WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_RBUTTONUP, WNDCLASSW, WS_EX_TOOLWINDOW,
                WS_EX_TOPMOST, WS_POPUP,
            },
        },
    },
    core::{PCWSTR, w},
};

/// Border of the selection, the application's teal.
const ACCENT: COLORREF = rgb(34, 184, 207);
/// Background and text of the size label and the hint.
const NIGHT: COLORREF = rgb(27, 30, 43);
const TEXT: COLORREF = rgb(232, 246, 248);
/// Opacity of the shade over the unselected screen, of 255.
const SHADE: u8 = 110;
/// A drag shorter than this in both directions, in pixels, counts as a click.
const CLICK: i32 = 4;

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}

thread_local! {
    static OVERLAY: RefCell<Option<Overlay>> = const { RefCell::new(None) };
}

fn failed(api: &'static str, code: u32) -> Reason {
    Reason::CallFailed { api, code }
}

/// Copies the screen and shows it for the user to select a region, with `hint` near the top
/// of the monitor under the pointer. Does nothing while a selection is under way. The
/// outcome arrives as a [`DesktopEvent::Capture`].
pub fn start(hint: &str) -> Result<(), Reason> {
    if OVERLAY.with(|overlay| overlay.borrow().is_some()) {
        return Ok(());
    }
    // The window and the copy use physical pixels, whatever awareness the thread has.
    // SAFETY: plain value argument; the previous context is restored below.
    let previous =
        unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let result = open(hint);
    if !previous.0.is_null() {
        // SAFETY: restores the context returned above.
        unsafe { SetThreadDpiAwarenessContext(previous) };
    }
    result
}

/// A memory device context with a top-down 32-bit DIB section selected into it.
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *const u8,
    width: i32,
    height: i32,
}

impl Surface {
    /// A surface of black pixels.
    fn new(width: i32, height: i32) -> Result<Self, Reason> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: the DC and the section belong to the surface and are released on drop or
        // here on failure; the bits stay valid while the section exists.
        unsafe {
            let dc = CreateCompatibleDC(None);
            if dc.is_invalid() {
                return Err(failed("CreateCompatibleDC", 0));
            }
            let mut bits = std::ptr::null_mut();
            let bitmap = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
            {
                Ok(bitmap) if !bits.is_null() => bitmap,
                result => {
                    if let Ok(bitmap) = result {
                        let _ = DeleteObject(bitmap.into());
                    }
                    let _ = DeleteDC(dc);
                    return Err(failed("CreateDIBSection", 0));
                }
            };
            let previous = SelectObject(dc, bitmap.into());
            Ok(Self {
                dc,
                bitmap,
                previous,
                bits: bits.cast(),
                width,
                height,
            })
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: restores the DC's original bitmap before deleting both, once.
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

/// The selection window and the screen it shows.
struct Overlay {
    window: HWND,
    shot: Surface,
    /// Off-screen copy of the window where each repaint composes its area, kept for the
    /// whole capture so that dragging allocates nothing.
    back: Surface,
    /// One black pixel, stretched over the unselected screen.
    shade: Surface,
    font: HFONT,
    /// Window pixels per 96 DPI pixel.
    scale: f32,
    /// Top-left corner of the virtual screen in screen coordinates.
    origin: POINT,
    /// Where the drag started, in window coordinates.
    anchor: Option<POINT>,
    pointer: POINT,
    /// The hint and its box, shown until a drag starts.
    hint: Vec<u16>,
    hint_box: Option<RECT>,
}

impl Drop for Overlay {
    fn drop(&mut self) {
        // SAFETY: the font belongs to the overlay and is selected into no DC by now.
        let _ = unsafe { DeleteObject(self.font.into()) };
    }
}

fn open(hint: &str) -> Result<(), Reason> {
    // SAFETY: plain value arguments.
    let (x, y, width, height) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    if width <= 0 || height <= 0 {
        return Err(failed("GetSystemMetrics", 0));
    }
    let shot = Surface::new(width, height)?;
    // SAFETY: the screen DC is released right after the copy into a surface of the same size.
    unsafe {
        let screen = GetDC(None);
        let copied = BitBlt(
            shot.dc,
            0,
            0,
            width,
            height,
            Some(screen),
            x,
            y,
            SRCCOPY | CAPTUREBLT,
        );
        ReleaseDC(None, screen);
        copied.map_err(|error| failed("BitBlt", error.code().0 as u32))?;
    }
    let back = Surface::new(width, height)?;
    let shade = Surface::new(1, 1)?;
    let class = w!("QqqMonitorCapture");
    // SAFETY: a null module name returns the running executable, which outlives the window;
    // the class name is static and the procedure lives for the whole process. Registering
    // the class again fails harmlessly.
    let window = unsafe {
        let module = GetModuleHandleW(PCWSTR::null())
            .map_err(|error| failed("GetModuleHandleW", error.code().0 as u32))?;
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: module.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
            ..Default::default()
        });
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class,
            w!("QQQ Monitor"),
            WS_POPUP,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(module.into()),
            None,
        )
        .map_err(|error| failed("CreateWindowExW", error.code().0 as u32))?
    };
    // SAFETY: only reads the window's DPI.
    let scale = unsafe { GetDpiForWindow(window) }.max(96) as f32 / 96.0;
    // SAFETY: plain value arguments and a static face name; the overlay deletes the font.
    let font = unsafe {
        CreateFontW(
            -(14.0 * scale).round() as i32,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Microsoft YaHei UI"),
        )
    };
    let mut pointer = POINT::default();
    // SAFETY: writes the pointer position to a local.
    let _ = unsafe { GetCursorPos(&mut pointer) };
    let mut overlay = Overlay {
        window,
        shot,
        back,
        shade,
        font,
        scale,
        origin: POINT { x, y },
        anchor: None,
        pointer: POINT {
            x: pointer.x - x,
            y: pointer.y - y,
        },
        hint: hint.encode_utf16().collect(),
        hint_box: None,
    };
    overlay.hint_box = overlay.place_hint();
    OVERLAY.with(|slot| *slot.borrow_mut() = Some(overlay));
    // Showing and activating send messages to the window, which finds its state ready.
    // SAFETY: the window belongs to this thread.
    unsafe {
        let _ = ShowWindow(window, SW_SHOW);
        let _ = SetForegroundWindow(window);
    }
    Ok(())
}

/// Runs `f` on the overlay unless it is missing or already in use further up the stack.
fn with_overlay<R>(f: impl FnOnce(&mut Overlay) -> R) -> Option<R> {
    OVERLAY.with(|slot| {
        let mut slot = slot.try_borrow_mut().ok()?;
        slot.as_mut().map(f)
    })
}

/// Ends the selection with the region `selection`, in window coordinates, or cancelled.
fn finish(selection: Option<RECT>) {
    let Some(overlay) = OVERLAY.with(|slot| slot.try_borrow_mut().ok()?.take()) else {
        return;
    };
    let end = selection
        .and_then(|region| overlay.crop(region))
        .map_or(CaptureEnd::Cancelled, CaptureEnd::Taken);
    // The state is taken out first, so the messages that destruction sends find none.
    // SAFETY: the window belongs to this thread.
    unsafe {
        let _ = ReleaseCapture();
        let _ = DestroyWindow(overlay.window);
    }
    drop(overlay);
    tray::push(DesktopEvent::Capture(end));
}

impl Overlay {
    /// The selected region in window coordinates, inside the window.
    fn selection(&self) -> Option<RECT> {
        let (a, p) = (self.anchor?, self.pointer);
        let (w, h) = (self.shot.width, self.shot.height);
        Some(RECT {
            left: a.x.min(p.x).clamp(0, w),
            top: a.y.min(p.y).clamp(0, h),
            right: a.x.max(p.x).clamp(0, w),
            bottom: a.y.max(p.y).clamp(0, h),
        })
    }

    /// Bounds of the monitor nearest to a window point, in window coordinates.
    fn monitor_at(&self, at: POINT) -> RECT {
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let point = POINT {
            x: at.x + self.origin.x,
            y: at.y + self.origin.y,
        };
        // SAFETY: the point is passed by value and `info` is writable with its size set.
        let found = unsafe {
            GetMonitorInfoW(MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST), &mut info)
        }
        .as_bool();
        if !found {
            return RECT {
                left: 0,
                top: 0,
                right: self.shot.width,
                bottom: self.shot.height,
            };
        }
        let r = info.rcMonitor;
        RECT {
            left: r.left - self.origin.x,
            top: r.top - self.origin.y,
            right: r.right - self.origin.x,
            bottom: r.bottom - self.origin.y,
        }
    }

    fn px(&self, points: f32) -> i32 {
        (points * self.scale).round() as i32
    }

    /// Size of `text` in the overlay's font.
    fn measure(&self, text: &[u16]) -> (i32, i32) {
        let mut rect = RECT::default();
        let mut text = text.to_vec();
        // SAFETY: the window DC is released after measuring; the font stays selected only
        // meanwhile.
        unsafe {
            let dc = GetDC(Some(self.window));
            let previous = SelectObject(dc, self.font.into());
            DrawTextW(
                dc,
                &mut text,
                &mut rect,
                DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
            );
            SelectObject(dc, previous);
            ReleaseDC(Some(self.window), dc);
        }
        (rect.right - rect.left, rect.bottom - rect.top)
    }

    /// Box of the hint, centered near the top of the monitor under the pointer.
    fn place_hint(&self) -> Option<RECT> {
        if self.hint.is_empty() {
            return None;
        }
        let (width, height) = self.measure(&self.hint);
        let (width, height) = (width + self.px(32.0), height + self.px(20.0));
        let monitor = self.monitor_at(self.pointer);
        let left = (monitor.left + monitor.right - width) / 2;
        let top = monitor.top + self.px(48.0);
        Some(RECT {
            left,
            top,
            right: left + width,
            bottom: top + height,
        })
    }

    /// Area to repaint when the selection changes from `old` to `new`: both selections with
    /// their border and size label.
    fn dirty(&self, old: Option<RECT>, new: Option<RECT>) -> RECT {
        let mut area = RECT {
            left: i32::MAX,
            top: i32::MAX,
            right: i32::MIN,
            bottom: i32::MIN,
        };
        for r in [old, new].into_iter().flatten() {
            area.left = area.left.min(r.left);
            area.top = area.top.min(r.top);
            area.right = area.right.max(r.right);
            area.bottom = area.bottom.max(r.bottom);
        }
        let label = self.px(48.0);
        RECT {
            left: area.left - self.px(8.0),
            top: area.top - label,
            right: area.right.max(area.left + self.px(200.0)) + self.px(8.0),
            bottom: area.bottom + label,
        }
    }

    /// Paints the window's `area` through the off-screen buffer, which has the window's
    /// coordinates.
    fn paint(&self, target: HDC, area: RECT) {
        let (width, height) = (area.right - area.left, area.bottom - area.top);
        if width <= 0 || height <= 0 {
            return;
        }
        let back = self.back.dc;
        // SAFETY: the surfaces belong to the overlay and outlive the calls; the font is
        // selected into the buffer only during the drawing below.
        unsafe {
            let _ = BitBlt(
                back,
                area.left,
                area.top,
                width,
                height,
                Some(self.shot.dc),
                area.left,
                area.top,
                SRCCOPY,
            );
            let selection = self.selection();
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: SHADE,
                AlphaFormat: 0,
            };
            for piece in outside(area, selection) {
                let _ = AlphaBlend(
                    back,
                    piece.left,
                    piece.top,
                    piece.right - piece.left,
                    piece.bottom - piece.top,
                    self.shade.dc,
                    0,
                    0,
                    1,
                    1,
                    blend,
                );
            }
            let font = SelectObject(back, self.font.into());
            SetBkMode(back, TRANSPARENT);
            SetTextColor(back, TEXT);
            match selection {
                Some(selection) => {
                    let line = self.px(2.0).max(1);
                    for edge in frame(selection, line) {
                        fill(back, edge, ACCENT);
                    }
                    self.size_label(back, selection, line);
                }
                None => {
                    if let Some(hint) = self.hint_box {
                        fill(back, hint, NIGHT);
                        for edge in frame(inset(hint, 1), 1) {
                            fill(back, edge, ACCENT);
                        }
                        let mut text = self.hint.clone();
                        let mut rect = hint;
                        DrawTextW(back, &mut text, &mut rect, centered());
                    }
                }
            }
            SelectObject(back, font);
            let _ = BitBlt(
                target,
                area.left,
                area.top,
                width,
                height,
                Some(back),
                area.left,
                area.top,
                SRCCOPY,
            );
        }
    }

    /// Width and height of the selection, above it or, without room there, inside it.
    fn size_label(&self, dc: HDC, selection: RECT, line: i32) {
        let mut text: Vec<u16> = format!(
            "{} × {}",
            selection.right - selection.left,
            selection.bottom - selection.top
        )
        .encode_utf16()
        .collect();
        let (width, height) = self.measure(&text);
        let (width, height) = (width + self.px(16.0), height + self.px(8.0));
        let gap = self.px(4.0);
        let mut top = selection.top - line - gap - height;
        if top < 0 {
            top = selection.top + gap;
        }
        let left = (selection.left - line).min(self.shot.width - width).max(0);
        let mut rect = RECT {
            left,
            top,
            right: left + width,
            bottom: top + height,
        };
        fill(dc, rect, NIGHT);
        // SAFETY: the DC is the paint buffer with the overlay's font selected.
        unsafe { DrawTextW(dc, &mut text, &mut rect, centered()) };
    }

    /// Pixels of a region in window coordinates, clipped to the screen copy.
    fn crop(&self, region: RECT) -> Option<Picture> {
        let (width, height) = (self.shot.width, self.shot.height);
        let left = region.left.clamp(0, width) as usize;
        let right = region.right.clamp(0, width) as usize;
        let top = region.top.clamp(0, height) as usize;
        let bottom = region.bottom.clamp(0, height) as usize;
        if right <= left || bottom <= top {
            return None;
        }
        // SAFETY: GDI has finished drawing into the section, which holds width × height
        // 32-bit pixels, top row first, for as long as the surface exists.
        let bits = unsafe {
            let _ = GdiFlush();
            std::slice::from_raw_parts(self.shot.bits, width as usize * height as usize * 4)
        };
        let columns = right - left;
        let mut rgba = Vec::with_capacity(columns * (bottom - top) * 4);
        for y in top..bottom {
            let row = &bits[(y * width as usize + left) * 4..][..columns * 4];
            for pixel in row.as_chunks::<4>().0 {
                rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
            }
        }
        Some(Picture {
            width: columns as u32,
            height: (bottom - top) as u32,
            rgba,
        })
    }
}

fn centered() -> DRAW_TEXT_FORMAT {
    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX
}

fn inset(r: RECT, by: i32) -> RECT {
    RECT {
        left: r.left + by,
        top: r.top + by,
        right: r.right - by,
        bottom: r.bottom - by,
    }
}

/// Four bars of `line` pixels around the outside of `r`.
fn frame(r: RECT, line: i32) -> [RECT; 4] {
    [
        RECT {
            left: r.left - line,
            top: r.top - line,
            right: r.right + line,
            bottom: r.top,
        },
        RECT {
            left: r.left - line,
            top: r.bottom,
            right: r.right + line,
            bottom: r.bottom + line,
        },
        RECT {
            left: r.left - line,
            top: r.top,
            right: r.left,
            bottom: r.bottom,
        },
        RECT {
            left: r.right,
            top: r.top,
            right: r.right + line,
            bottom: r.bottom,
        },
    ]
}

/// Parts of `area` outside `selection`: everything without a selection.
fn outside(area: RECT, selection: Option<RECT>) -> Vec<RECT> {
    let Some(s) = selection else {
        return vec![area];
    };
    let middle_top = s.top.clamp(area.top, area.bottom);
    let middle_bottom = s.bottom.clamp(area.top, area.bottom);
    [
        RECT {
            bottom: middle_top,
            ..area
        },
        RECT {
            top: middle_bottom,
            ..area
        },
        RECT {
            top: middle_top,
            bottom: middle_bottom,
            right: s.left.clamp(area.left, area.right),
            ..area
        },
        RECT {
            top: middle_top,
            bottom: middle_bottom,
            left: s.right.clamp(area.left, area.right),
            ..area
        },
    ]
    .into_iter()
    .filter(|r| r.right > r.left && r.bottom > r.top)
    .collect()
}

fn fill(dc: HDC, rect: RECT, color: COLORREF) {
    // SAFETY: the brush is created and deleted around one fill of a DC that is in use.
    unsafe {
        let brush = CreateSolidBrush(color);
        FillRect(dc, &rect, brush);
        let _ = DeleteObject(brush.into());
    }
}

/// Window coordinates of a mouse message.
fn point(lparam: LPARAM) -> POINT {
    POINT {
        x: (lparam.0 & 0xFFFF) as u16 as i16 as i32,
        y: ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32,
    }
}

fn invalidate(window: HWND, area: Option<RECT>) {
    // SAFETY: the rectangle outlives the call, which only marks the window for painting.
    let _ = unsafe { InvalidateRect(Some(window), area.as_ref().map(|r| r as *const RECT), false) };
}

/// Window procedure of the selection window. Each handler borrows the overlay only for its
/// own bookkeeping and calls into the system after releasing it, since those calls may
/// send messages back here.
unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_LBUTTONDOWN => {
            let at = point(lparam);
            let hint = with_overlay(|overlay| {
                overlay.anchor = Some(at);
                overlay.pointer = at;
                overlay.hint_box.take()
            });
            if let Some(Some(hint)) = hint {
                invalidate(window, Some(hint));
            }
            // SAFETY: the window belongs to this thread; capture ends in `finish`.
            unsafe { SetCapture(window) };
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let at = point(lparam);
            let area = with_overlay(|overlay| {
                let old = overlay.selection();
                overlay.pointer = at;
                old.map(|old| overlay.dirty(Some(old), overlay.selection()))
            });
            if let Some(Some(area)) = area {
                invalidate(window, Some(area));
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let at = point(lparam);
            let region = with_overlay(|overlay| {
                let anchor = overlay.anchor?;
                overlay.pointer = at;
                if (at.x - anchor.x).abs() < CLICK && (at.y - anchor.y).abs() < CLICK {
                    Some(overlay.monitor_at(at))
                } else {
                    overlay.selection()
                }
            });
            if let Some(Some(region)) = region {
                finish(Some(region));
            }
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            // The right button drops a drag under way, and otherwise leaves.
            if with_overlay(|overlay| overlay.anchor.take().is_some()) == Some(true) {
                // SAFETY: releases this thread's own mouse capture.
                let _ = unsafe { ReleaseCapture() };
                invalidate(window, None);
            } else {
                finish(None);
            }
            LRESULT(0)
        }
        WM_KEYDOWN if wparam.0 == usize::from(VK_ESCAPE.0) => {
            finish(None);
            LRESULT(0)
        }
        // Another window took the foreground, so the frozen screen is out of date.
        WM_ACTIVATE if wparam.0 as u32 & 0xFFFF == WA_INACTIVE => {
            finish(None);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        // The window keeps covering the virtual screen at any DPI.
        WM_DPICHANGED => LRESULT(0),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            // SAFETY: begins and ends painting this window; drawing sends no messages, so
            // the shared borrow cannot meet a handler that changes the overlay.
            unsafe {
                let dc = BeginPaint(window, &mut paint);
                OVERLAY.with(|slot| {
                    if let Ok(slot) = slot.try_borrow()
                        && let Some(overlay) = slot.as_ref()
                    {
                        overlay.paint(dc, paint.rcPaint);
                    }
                });
                let _ = EndPaint(window, &paint);
            }
            LRESULT(0)
        }
        // SAFETY: forwards the unmodified message of this window.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
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
    fn the_shade_covers_the_area_around_the_selection() {
        let area = rect(0, 0, 100, 100);
        assert_eq!(outside(area, None), vec![area]);
        let pieces = outside(area, Some(rect(10, 20, 30, 40)));
        assert_eq!(
            pieces,
            vec![
                rect(0, 0, 100, 20),
                rect(0, 40, 100, 100),
                rect(0, 20, 10, 40),
                rect(30, 20, 100, 40),
            ]
        );
        let covered: i32 = pieces
            .iter()
            .map(|r| (r.right - r.left) * (r.bottom - r.top))
            .sum();
        assert_eq!(covered, 100 * 100 - 20 * 20);
        // A selection beyond the painted area leaves it whole.
        assert_eq!(
            outside(rect(50, 50, 60, 60), Some(rect(0, 0, 10, 10))),
            vec![rect(50, 50, 60, 60)]
        );
    }
}
