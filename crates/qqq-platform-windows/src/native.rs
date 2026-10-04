use qqq_domain::CollectorError;
use std::{collections::BTreeMap, mem::size_of, ptr};
use windows::{
    Win32::{
        NetworkManagement::{IpHelper::*, Ndis::IfOperStatusUp},
        Storage::FileSystem::{GetDriveTypeW, GetVolumeNameForVolumeMountPointW},
        System::Performance::*,
        UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
    },
    core::{HSTRING, PCWSTR, w},
};

fn error(api: &str, status: u32) -> CollectorError {
    CollectorError(format!("{api} 失败，错误码 0x{status:08X}"))
}
fn text(value: &[u16]) -> String {
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
        return Err(CollectorError("GetIfTable2 返回空表".into()));
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

pub fn show_error(message: &str) {
    // SAFETY: no owner, and both strings remain valid for the duration of the modal call.
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            w!("QQQ Monitor 启动失败"),
            MB_OK | MB_ICONERROR,
        );
    }
}

pub struct DiskQuery {
    handle: PDH_HQUERY,
    read: PDH_HCOUNTER,
    write: PDH_HCOUNTER,
}
impl Drop for DiskQuery {
    fn drop(&mut self) {
        /* SAFETY: owned query, released on its creating thread exactly once. */
        unsafe {
            PdhCloseQuery(self.handle);
        }
    }
}
type Values = BTreeMap<String, Option<f64>>;
impl DiskQuery {
    pub fn new() -> Result<Self, CollectorError> {
        let mut handle = PDH_HQUERY::default();
        // SAFETY: output pointer is valid; None requests live system counters.
        let status = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut handle) };
        if status != 0 {
            return Err(error("PdhOpenQueryW", status));
        }
        let mut query = Self {
            handle,
            read: PDH_HCOUNTER::default(),
            write: PDH_HCOUNTER::default(),
        };
        for (path, counter) in [
            (w!(r"\PhysicalDisk(*)\Disk Read Bytes/sec"), &mut query.read),
            (
                w!(r"\PhysicalDisk(*)\Disk Write Bytes/sec"),
                &mut query.write,
            ),
        ] {
            // SAFETY: query is open, static UTF-16 path is valid, and output handle storage lives through call.
            let status = unsafe { PdhAddEnglishCounterW(query.handle, path, 0, counter) };
            if status != 0 {
                return Err(error("PdhAddEnglishCounterW", status));
            }
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
    pub fn values(&self) -> Result<(Values, Values), CollectorError> {
        Ok((self.array(self.read)?, self.array(self.write)?))
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
            if bytes as usize > 1024 * 1024 {
                return Err(CollectorError("性能计数器缓冲区异常".into()));
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
                return Err(CollectorError("性能计数器数组长度无效".into()));
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
                    return Err(CollectorError("性能计数器名称指针无效".into()));
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
        Err(CollectorError(
            "设备变化频繁，暂时无法读取磁盘计数器".into(),
        ))
    }
}
