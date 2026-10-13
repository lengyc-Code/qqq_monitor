use qqq_domain::{CollectorError, Reason};
use std::{collections::BTreeMap, mem::size_of, ptr};
use windows::{
    Win32::{
        Foundation::{ERROR_CANCELLED, HWND, LPARAM, POINT, RECT, WPARAM},
        Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME},
        Graphics::{
            Dxgi::{
                CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, IDXGIFactory1,
            },
            Gdi::{
                GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL, MONITORINFO,
                MonitorFromPoint,
            },
        },
        NetworkManagement::{IpHelper::*, Ndis::IfOperStatusUp},
        Storage::FileSystem::{GetDriveTypeW, GetVolumeNameForVolumeMountPointW},
        System::{LibraryLoader::GetModuleHandleW, Performance::*},
        UI::{
            HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON},
            Shell::{SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::{
                ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, MB_ICONERROR, MB_OK,
                MessageBoxW, SM_CXICON, SM_CXSMICON, SW_SHOWNORMAL, SendMessageW, WM_SETICON,
            },
        },
    },
    core::{HSTRING, PCWSTR, PWSTR, w},
};

fn error(api: &'static str, code: u32) -> CollectorError {
    CollectorError(Reason::CallFailed { api, code })
}
fn bad_data(api: &'static str) -> CollectorError {
    CollectorError(Reason::BadData { api })
}
pub(crate) fn text(value: &[u16]) -> String {
    String::from_utf16_lossy(&value[..value.iter().position(|v| *v == 0).unwrap_or(value.len())])
}

pub struct NetworkRow {
    pub luid: u64,
    pub guid: String,
    pub name: String,
    pub description: String,
    pub connected: bool,
    pub physical: bool,
    pub link_speed: u64,
    pub received: u64,
    pub sent: u64,
}

struct InterfaceTable(*mut MIB_IF_TABLE2);
impl Drop for InterfaceTable {
    fn drop(&mut self) {
        /* SAFETY: table is allocated by GetIfTable2 and freed once after all reads. */
        unsafe {
            FreeMibTable(self.0.cast());
        }
    }
}

pub fn network_rows() -> Result<Vec<NetworkRow>, CollectorError> {
    let mut pointer = ptr::null_mut();
    // SAFETY: API writes an owned MIB table pointer to valid local storage.
    let status = unsafe { GetIfTable2(&mut pointer) };
    if status.0 != 0 {
        return Err(error("GetIfTable2", status.0));
    }
    if pointer.is_null() {
        return Err(bad_data("GetIfTable2"));
    }
    let table = InterfaceTable(pointer);
    // SAFETY: successful API call allocates NumEntries contiguous rows after the header.
    let rows = unsafe {
        std::slice::from_raw_parts(
            ptr::addr_of!((*table.0).Table).cast::<MIB_IF_ROW2>(),
            (*table.0).NumEntries as usize,
        )
    };
    let mut result = Vec::new();
    for row in rows.iter().filter(|row| {
        // Filter modules duplicate their underlying adapter's traffic. Endpoint
        // interfaces do not provide network connectivity (MIB_IF_ROW2 flags).
        row.Type != IF_TYPE_SOFTWARE_LOOPBACK
            && row.InterfaceAndOperStatusFlags._bitfield & (0b10 | 0b1000_0000) == 0
    }) {
        // SAFETY: NET_LUID's documented Value member is a 64-bit interface identifier.
        let luid = unsafe { row.InterfaceLuid.Value };
        result.push(NetworkRow {
            luid,
            guid: format!("{:?}", row.InterfaceGuid),
            name: text(&row.Alias),
            description: text(&row.Description),
            connected: row.OperStatus == IfOperStatusUp,
            physical: row.InterfaceAndOperStatusFlags._bitfield & 0b101 == 0b101,
            link_speed: row.ReceiveLinkSpeed.max(row.TransmitLinkSpeed),
            received: row.InOctets,
            sent: row.OutOctets,
        });
    }
    Ok(result)
}

pub fn local_volume_id(mount: &str) -> Option<String> {
    let mount = HSTRING::from(mount);
    let mut buffer = [0_u16; 1024];
    // SAFETY: mount is NUL-terminated HSTRING; output slice supplies its length to Win32.
    unsafe {
        if ![2, 3, 6].contains(&GetDriveTypeW(&mount)) {
            return None;
        }
        GetVolumeNameForVolumeMountPointW(&mount, &mut buffer).ok()?;
    }
    Some(text(&buffer))
}

/// Starts this executable again with administrator rights and `arguments`, once the user
/// confirms the system's prompt; declining it fails with [`Reason::Declined`].
pub fn restart_elevated(arguments: &str) -> Result<(), Reason> {
    let path = std::env::current_exe().map_err(|error| Reason::CallFailed {
        api: "GetModuleFileNameW",
        code: error.raw_os_error().unwrap_or_default() as u32,
    })?;
    let file = HSTRING::from(path.as_os_str());
    let arguments = HSTRING::from(arguments);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(arguments.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: the structure has its size set and the strings it points to outlive the call,
    // which waits while the system asks the user.
    unsafe { ShellExecuteExW(&mut info) }.map_err(|error| {
        if error.code() == ERROR_CANCELLED.to_hresult() {
            Reason::Declined
        } else {
            Reason::CallFailed {
                api: "ShellExecuteExW",
                code: error.code().0 as u32,
            }
        }
    })
}

pub fn show_error(title: &str, message: &str) {
    // SAFETY: no owner, and both strings remain valid for the duration of the modal call.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            &HSTRING::from(title),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// Display languages of the user as BCP 47 tags such as `ja-JP`, most preferred first;
/// empty when the system cannot report them.
pub fn ui_languages() -> Vec<String> {
    let (mut count, mut length) = (0_u32, 0_u32);
    // SAFETY: without a buffer the call only writes the language count and the required
    // buffer length to valid locals.
    if unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, None, &mut length) }
        .is_err()
    {
        return Vec::new();
    }
    let mut buffer = vec![0_u16; length as usize];
    // SAFETY: the buffer holds the `length` UTF-16 units the first call asked for and
    // outlives the call.
    if unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut count,
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut length,
        )
    }
    .is_err()
    {
        return Vec::new();
    }
    // NUL-terminated names followed by an empty name.
    buffer
        .split(|unit| *unit == 0)
        .filter(|name| !name.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

/// Live PDH query over wildcard counter paths; values are read back by path index.
pub struct CounterQuery {
    handle: PDH_HQUERY,
    counters: Vec<PDH_HCOUNTER>,
}
impl Drop for CounterQuery {
    fn drop(&mut self) {
        /* SAFETY: owned query, released on its creating thread exactly once. */
        unsafe {
            PdhCloseQuery(self.handle);
        }
    }
}
pub type Values = BTreeMap<String, Option<f64>>;
impl CounterQuery {
    pub fn new(paths: &[&str]) -> Result<Self, CollectorError> {
        let mut handle = PDH_HQUERY::default();
        // SAFETY: output pointer is valid; None requests live system counters.
        let status = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut handle) };
        if status != 0 {
            return Err(error("PdhOpenQueryW", status));
        }
        let mut query = Self {
            handle,
            counters: Vec::with_capacity(paths.len()),
        };
        for path in paths {
            let mut counter = PDH_HCOUNTER::default();
            // SAFETY: query is open, the NUL-terminated path and output handle storage live through the call.
            let status = unsafe {
                PdhAddEnglishCounterW(query.handle, &HSTRING::from(*path), 0, &mut counter)
            };
            if status != 0 {
                return Err(error("PdhAddEnglishCounterW", status));
            }
            query.counters.push(counter);
        }
        Ok(query)
    }
    pub fn collect(&self) -> Result<(), CollectorError> {
        // SAFETY: query handle remains owned and valid on this thread.
        let status = unsafe { PdhCollectQueryData(self.handle) };
        if status == 0 {
            Ok(())
        } else {
            Err(error("PdhCollectQueryData", status))
        }
    }
    /// Instance values of the counter added at `index`; invalid samples are `None`.
    pub fn values(&self, index: usize) -> Result<Values, CollectorError> {
        self.array(self.counters[index])
    }
    fn array(&self, counter: PDH_HCOUNTER) -> Result<Values, CollectorError> {
        let mut bytes = 0_u32;
        let mut count = 0_u32;
        // SAFETY: first call only discovers the required buffer size.
        let status = unsafe {
            PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut bytes, &mut count, None)
        };
        if status != PDH_MORE_DATA && status != 0 {
            return Err(error("PdhGetFormattedCounterArrayW", status));
        }
        if bytes == 0 {
            return Ok(BTreeMap::new());
        }
        for _ in 0..3 {
            // GPU engine counters list one instance per process and engine.
            if bytes as usize > 4 * 1024 * 1024 {
                return Err(bad_data("PdhGetFormattedCounterArrayW"));
            }
            // u64 provides at least the alignment required by the PDH item structure.
            let mut buffer = vec![0_u64; (bytes as usize).div_ceil(size_of::<u64>())];
            let capacity = buffer.len() * size_of::<u64>();
            bytes = capacity as u32;
            // SAFETY: allocated, aligned writable storage covers bytes and stays alive during parsing.
            let status = unsafe {
                PdhGetFormattedCounterArrayW(
                    counter,
                    PDH_FMT_DOUBLE,
                    &mut bytes,
                    &mut count,
                    Some(buffer.as_mut_ptr().cast()),
                )
            };
            if status == PDH_MORE_DATA {
                continue;
            }
            if status != 0 {
                return Err(error("PdhGetFormattedCounterArrayW", status));
            }
            if count as usize > capacity / size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>() {
                return Err(bad_data("PdhGetFormattedCounterArrayW"));
            }
            // SAFETY: API initialized count items, and bounds were checked above.
            let items = unsafe {
                std::slice::from_raw_parts(
                    buffer.as_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>(),
                    count as usize,
                )
            };
            let begin = buffer.as_ptr() as usize;
            let end = begin + capacity;
            let mut values = BTreeMap::new();
            for item in items {
                let pointer = item.szName.0 as usize;
                if pointer < begin || pointer >= end || !pointer.is_multiple_of(2) {
                    return Err(bad_data("PdhGetFormattedCounterArrayW"));
                }
                // SAFETY: name pointer is inside the live buffer; scan is bounded by its allocation.
                let name =
                    text(unsafe { std::slice::from_raw_parts(item.szName.0, (end - pointer) / 2) });
                if name == "_Total" {
                    continue;
                }
                // SAFETY: PDH_FMT_DOUBLE was requested, selecting the union's doubleValue member.
                let value = unsafe { item.FmtValue.Anonymous.doubleValue };
                values.insert(
                    name,
                    if [PDH_CSTATUS_VALID_DATA, PDH_CSTATUS_NEW_DATA]
                        .contains(&item.FmtValue.CStatus)
                        && value.is_finite()
                        && value >= 0.0
                    {
                        Some(value)
                    } else {
                        None
                    },
                );
            }
            return Ok(values);
        }
        Err(CollectorError(Reason::InstancesChanging))
    }
}

pub struct GpuAdapter {
    /// Adapter LUID as `HighPart << 32 | LowPart`, matching PDH instance names.
    pub luid: u64,
    pub name: String,
    /// PCI vendor ID.
    pub vendor: u32,
    /// PCI device, subsystem and revision IDs.
    pub device: u32,
    pub subsystem: u32,
    pub revision: u32,
    pub dedicated: u64,
    pub shared: u64,
    pub software: bool,
}

pub fn gpu_adapters() -> Result<Vec<GpuAdapter>, CollectorError> {
    let failed = |api: &'static str, e: windows::core::Error| error(api, e.code().0 as u32);
    // SAFETY: factory creation has no preconditions; the COM wrapper releases it on drop.
    let factory: IDXGIFactory1 =
        unsafe { CreateDXGIFactory1() }.map_err(|e| failed("CreateDXGIFactory1", e))?;
    let mut adapters = Vec::new();
    for index in 0.. {
        // SAFETY: indices are enumerated in order until DXGI_ERROR_NOT_FOUND; wrappers release adapters.
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(e) => return Err(failed("EnumAdapters1", e)),
        };
        // SAFETY: the adapter is alive and GetDesc1 fills a plain descriptor value.
        let desc = unsafe { adapter.GetDesc1() }.map_err(|e| failed("GetDesc1", e))?;
        adapters.push(GpuAdapter {
            luid: (desc.AdapterLuid.HighPart as u32 as u64) << 32 | desc.AdapterLuid.LowPart as u64,
            name: text(&desc.Description),
            vendor: desc.VendorId,
            device: desc.DeviceId,
            subsystem: desc.SubSysId,
            revision: desc.Revision,
            dedicated: desc.DedicatedVideoMemory as u64,
            shared: desc.SharedSystemMemory as u64,
            software: desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0,
        });
    }
    Ok(adapters)
}

fn point(at: [f32; 2]) -> POINT {
    POINT {
        x: at[0].round() as i32,
        y: at[1].round() as i32,
    }
}

/// Bounds and work area of the monitor nearest to a point, as `[left, top, right, bottom]`
/// in physical pixels of the virtual screen.
pub fn monitor_at(at: [f32; 2]) -> Option<([f32; 4], [f32; 4])> {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: the point is passed by value and `info` is a writable MONITORINFO with its size set.
    unsafe {
        let monitor = MonitorFromPoint(point(at), MONITOR_DEFAULTTONEAREST);
        if monitor.is_invalid() || !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
    }
    let edges = |r: RECT| [r.left as f32, r.top as f32, r.right as f32, r.bottom as f32];
    Some((edges(info.rcMonitor), edges(info.rcWork)))
}

/// Whether a physical-pixel point lies on any monitor.
pub fn covers_point(at: [f32; 2]) -> bool {
    // SAFETY: plain value arguments; the returned handle is only compared.
    !unsafe { MonitorFromPoint(point(at), MONITOR_DEFAULTTONULL) }.is_invalid()
}

/// Whether a mouse button is physically held, including during a system window move.
pub fn mouse_button_held() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON]
        .iter()
        // SAFETY: reads the asynchronous key state of a valid virtual-key code.
        .any(|key| unsafe { GetAsyncKeyState(key.0 as i32) } < 0)
}

/// Resource ID of the application icon that `build.rs` embeds from `assets/qqq_monitor.rc`.
pub(crate) const APP_ICON: u16 = 1;

/// Sets the title-bar and taskbar icons of a window from the icon embedded in the executable,
/// loading for each the size the system uses at the window's DPI.
pub fn set_window_icon(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut _);
    // SAFETY: only reads the handle; an invalid window yields 0.
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 {
        return;
    }
    // SAFETY: a null module name returns the running executable, which outlives the window.
    let Ok(module) = (unsafe { GetModuleHandleW(PCWSTR::null()) }) else {
        return;
    };
    for (kind, metric) in [(ICON_BIG, SM_CXICON), (ICON_SMALL, SM_CXSMICON)] {
        // SAFETY: plain value arguments.
        let size = unsafe { GetSystemMetricsForDpi(metric, dpi) };
        // SAFETY: the integer resource ID is passed as MAKEINTRESOURCE; Win32 picks the closest
        // embedded size and scales it to `size`.
        let Ok(icon) = (unsafe {
            LoadImageW(
                Some(module.into()),
                PCWSTR(APP_ICON as usize as *const u16),
                IMAGE_ICON,
                size,
                size,
                LR_DEFAULTCOLOR,
            )
        }) else {
            continue;
        };
        // The window shows the icon for the rest of the process, so the handle stays alive
        // and is reclaimed at exit.
        // SAFETY: WM_SETICON only stores the icon handle on the window.
        unsafe {
            SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(kind as usize)),
                Some(LPARAM(icon.0 as isize)),
            )
        };
    }
}
