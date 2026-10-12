//! The shell program a session keeps running on the host, and the devices and readings
//! read from its output. The program uses only POSIX shell, `uname`, `getconf`, `cat`
//! and `df`, so busybox-based systems run it as well.

use qqq_domain::*;
use std::{collections::HashMap, time::Instant};

/// Sent to `sh -s`. The loop prints `@@ ready` once the whole program is read, then
/// answers each command line with sections headed `@@ name` and ends with `@@ end`:
/// `i` describes the host, `s` prints the counters and file systems. Printing the marker
/// in the loop condition keeps the shell from reading a command as part of the program.
pub(crate) const SCRIPT: &str = r#"export LC_ALL=C
i() {
  echo "@@ info"
  echo "hostname $(uname -n 2>/dev/null)"
  echo "kernel $(uname -sr 2>/dev/null)"
  echo "arch $(uname -m 2>/dev/null)"
  echo "cores $(getconf _NPROCESSORS_ONLN 2>/dev/null)"
  echo "@@ os-release"
  cat /etc/os-release 2>/dev/null || cat /usr/lib/os-release 2>/dev/null
  echo "@@ cpuinfo"
  while IFS= read -r l; do
    case $l in "model name"*|Model*|Hardware*|"cpu model"*|Processor*) printf '%s\n' "$l" ;; esac
  done < /proc/cpuinfo
}
s() {
  for f in uptime loadavg stat meminfo net/dev net/route diskstats; do
    echo "@@ $f"
    cat "/proc/$f" 2>/dev/null
  done
  echo "@@ block"
  for b in /sys/block/*; do echo "${b##*/}"; done
  echo "@@ physical"
  for n in /sys/class/net/*; do [ -e "$n/device" ] && echo "${n##*/}"; done
  echo "@@ df"
  d=$(df -kPTl 2>/dev/null)
  [ -n "$d" ] || d=$(df -kPT 2>/dev/null)
  [ -n "$d" ] || d=$(df -kP 2>/dev/null)
  printf '%s\n' "$d"
}
r=ready
while echo "@@ $r"; r=end; read -r c; do
  case $c in
    i) i ;;
    s) s ;;
    *) exit 0 ;;
  esac
done
"#;

/// Line that ends the program's start, and the answer to each command.
pub(crate) const READY: &str = "@@ ready";
pub(crate) const END: &str = "@@ end";

/// Block devices that are not whole physical disks, or that repeat their I/O.
const VIRTUAL_DISKS: [&str; 8] = ["loop", "ram", "zram", "dm-", "md", "sr", "fd", "nbd"];
/// File systems without storage of their own, and read-only images that are always full.
const VIRTUAL_FILE_SYSTEMS: [&str; 28] = [
    "tmpfs",
    "devtmpfs",
    "squashfs",
    "overlay",
    "ramfs",
    "rootfs",
    "efivarfs",
    "devfs",
    "proc",
    "sysfs",
    "cgroup",
    "cgroup2",
    "autofs",
    "nsfs",
    "tracefs",
    "debugfs",
    "securityfs",
    "pstore",
    "bpf",
    "fusectl",
    "configfs",
    "hugetlbfs",
    "mqueue",
    "binfmt_misc",
    "rpc_pipefs",
    "nfsd",
    "iso9660",
    "none",
];
/// Mount points of system and container internals.
const SYSTEM_MOUNTS: [&str; 7] = [
    "/proc",
    "/sys",
    "/dev",
    "/run",
    "/snap",
    "/var/lib/docker",
    "/var/lib/kubelet",
];

/// Lines of each `@@ name` section.
fn sections(text: &str) -> HashMap<&str, Vec<&str>> {
    let mut map: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut current = None;
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("@@ ") {
            let name = name.trim();
            map.entry(name).or_default();
            current = Some(name);
        } else if let Some(name) = current {
            map.entry(name).or_default().push(line);
        }
    }
    map
}

/// What the host is, read once per connection.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct HostInfo {
    pub hostname: String,
    pub kernel: String,
    pub arch: String,
    pub os: String,
    pub cores: Option<u32>,
    pub model: String,
}

pub(crate) fn parse_info(text: &str) -> HostInfo {
    let sections = sections(text);
    let lines = |name| sections.get(name).map(Vec::as_slice).unwrap_or_default();
    let mut info = HostInfo::default();
    for line in lines("info") {
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        let value = value.trim().to_string();
        match key {
            "hostname" => info.hostname = value,
            "kernel" => info.kernel = value,
            "arch" => info.arch = value,
            "cores" => info.cores = value.parse().ok().filter(|n| *n > 0),
            _ => {}
        }
    }
    let release: HashMap<_, _> = lines("os-release")
        .iter()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim(), value.trim().trim_matches(['"', '\''])))
        .collect();
    info.os = match (release.get("PRETTY_NAME"), release.get("NAME")) {
        (Some(pretty), _) if !pretty.is_empty() => pretty.to_string(),
        (_, Some(name)) => format!("{name} {}", release.get("VERSION").unwrap_or(&""))
            .trim()
            .to_string(),
        _ => "Linux".into(),
    };
    let cpuinfo: Vec<_> = lines("cpuinfo")
        .iter()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim(), value.trim()))
        .filter(|(_, value)| !value.is_empty())
        .collect();
    // x86 names the model; ARM boards name the board or SoC instead.
    info.model = ["model name", "Model", "Hardware", "cpu model", "Processor"]
        .iter()
        .find_map(|wanted| cpuinfo.iter().find(|(key, _)| key == wanted))
        .map(|(_, value)| value.to_string())
        .unwrap_or_default();
    info
}

/// Processor time in clock ticks, from a `/proc/stat` CPU line.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct CpuTimes {
    pub busy: u64,
    pub iowait: u64,
    pub total: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Net {
    pub name: String,
    pub received: u64,
    pub sent: u64,
    pub physical: bool,
    pub default_route: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Volume {
    pub mount: String,
    pub source: String,
    pub file_system: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
}

/// Counters and file systems of one `s` answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Raw {
    /// Seconds since boot; the clock for every rate.
    pub uptime: f64,
    pub load: Option<[f64; 3]>,
    pub tasks: Option<u64>,
    /// The total first, then each logical processor by index.
    pub cpu: CpuTimes,
    pub cores: Vec<(usize, CpuTimes)>,
    /// `MemTotal`, available memory, `SwapTotal` and `SwapFree`, in bytes.
    pub memory: Option<(u64, u64, u64, u64)>,
    pub nets: Vec<Net>,
    /// Sectors of 512 bytes read and written by each whole disk.
    pub disks: Vec<(String, u64, u64)>,
    pub volumes: Vec<Volume>,
}

fn cpu_times(fields: &[&str]) -> Option<CpuTimes> {
    let value = |i: usize| {
        fields
            .get(i)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };
    // user nice system idle iowait irq softirq steal; guest time is part of user time.
    fields.get(3)?.parse::<u64>().ok()?;
    let idle = value(3) + value(4);
    let busy = value(0) + value(1) + value(2) + value(5) + value(6) + value(7);
    Some(CpuTimes {
        busy,
        iowait: value(4),
        total: busy + idle,
    })
}

pub(crate) fn parse_sample(text: &str) -> Result<Raw, Reason> {
    let sections = sections(text);
    let lines = |name| sections.get(name).map(Vec::as_slice).unwrap_or_default();
    let mut raw = Raw {
        uptime: lines("uptime")
            .first()
            .and_then(|line| line.split_whitespace().next()?.parse().ok())
            .ok_or(Reason::NotLinux)?,
        ..Default::default()
    };
    if let Some(line) = lines("loadavg").first() {
        let fields: Vec<_> = line.split_whitespace().collect();
        let load = |i: usize| fields.get(i).and_then(|v| v.parse::<f64>().ok());
        raw.load = load(0)
            .zip(load(1))
            .zip(load(2))
            .map(|((a, b), c)| [a, b, c]);
        raw.tasks = fields
            .get(3)
            .and_then(|v| v.split_once('/'))
            .and_then(|(_, total)| total.parse().ok());
    }
    let mut total = None;
    for line in lines("stat") {
        let mut fields = line.split_whitespace();
        let Some(name) = fields.next() else { continue };
        let Some(rest) = name.strip_prefix("cpu") else {
            continue;
        };
        let Some(times) = cpu_times(&fields.collect::<Vec<_>>()) else {
            continue;
        };
        if rest.is_empty() {
            total = Some(times);
        } else if let Ok(index) = rest.parse() {
            raw.cores.push((index, times));
        }
    }
    raw.cpu = total.ok_or(Reason::NotLinux)?;
    let meminfo: HashMap<_, _> = lines("meminfo")
        .iter()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            let kib: u64 = value.split_whitespace().next()?.parse().ok()?;
            Some((key, kib * 1024))
        })
        .collect();
    if let Some(&total) = meminfo.get("MemTotal") {
        // Kernels before 3.14 lack MemAvailable.
        let available = meminfo.get("MemAvailable").copied().unwrap_or_else(|| {
            ["MemFree", "Buffers", "Cached"]
                .iter()
                .filter_map(|key| meminfo.get(key))
                .sum()
        });
        let swap = |key| meminfo.get(key).copied().unwrap_or(0);
        raw.memory = Some((
            total,
            available.min(total),
            swap("SwapTotal"),
            swap("SwapFree"),
        ));
    }
    let physical: Vec<_> = lines("physical").iter().map(|l| l.trim()).collect();
    let default_route = lines("net/route")
        .iter()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            let metric: u64 = fields.get(6)?.parse().ok()?;
            (fields.get(1) == Some(&"00000000") && fields.get(7) == Some(&"00000000"))
                .then_some((metric, fields[0]))
        })
        .min()
        .map(|(_, name)| name);
    for line in lines("net/dev") {
        let Some((name, counters)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        let counters: Vec<u64> = counters
            .split_whitespace()
            .filter_map(|v| v.parse().ok())
            .collect();
        // Container peers come and go by the dozen and repeat their bridge's traffic.
        if name.is_empty() || name == "lo" || name.starts_with("veth") || counters.len() < 16 {
            continue;
        }
        raw.nets.push(Net {
            name: name.into(),
            received: counters[0],
            sent: counters[8],
            physical: physical.contains(&name),
            default_route: default_route == Some(name),
        });
    }
    let block: Vec<_> = lines("block").iter().map(|l| l.trim()).collect();
    for line in lines("diskstats") {
        let fields: Vec<_> = line.split_whitespace().collect();
        let (Some(name), Some(read), Some(written)) = (
            fields.get(2),
            fields.get(5).and_then(|v| v.parse().ok()),
            fields.get(9).and_then(|v| v.parse().ok()),
        ) else {
            continue;
        };
        if block.contains(name) && !VIRTUAL_DISKS.iter().any(|p| name.starts_with(p)) {
            raw.disks.push((name.to_string(), read, written));
        }
    }
    raw.volumes = volumes(lines("df"));
    Ok(raw)
}

/// Local file systems from `df -kP`, with or without the type column; each source
/// device counts once, at its shortest mount point.
fn volumes(lines: &[&str]) -> Vec<Volume> {
    let Some(header) = lines.first() else {
        return Vec::new();
    };
    let typed = header.split_whitespace().nth(1) == Some("Type");
    let mut volumes: Vec<Volume> = Vec::new();
    for line in &lines[1..] {
        let fields: Vec<_> = line.split_whitespace().collect();
        let skip = usize::from(typed);
        if fields.len() < 6 + skip {
            continue;
        }
        let number = |i: usize| fields[i + skip].parse::<u64>().ok().map(|kib| kib * 1024);
        let (Some(total), Some(used), Some(available)) = (number(1), number(2), number(3)) else {
            continue;
        };
        // A mount point may contain spaces.
        let mount = line
            .split_whitespace()
            .skip(5 + skip)
            .collect::<Vec<_>>()
            .join(" ");
        let file_system = if typed { fields[1] } else { "" };
        let source = fields[0];
        let system = |prefix: &&str| {
            mount == *prefix
                || mount
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('/'))
        };
        if total == 0
            || VIRTUAL_FILE_SYSTEMS.contains(&file_system)
            || VIRTUAL_FILE_SYSTEMS.contains(&source)
            || source.starts_with("/dev/loop")
            || (SYSTEM_MOUNTS.iter().any(system) && !mount.starts_with("/run/media/"))
        {
            continue;
        }
        let volume = Volume {
            mount,
            source: source.into(),
            file_system: file_system.into(),
            total,
            used,
            available,
        };
        match volumes
            .iter_mut()
            .find(|v| v.source == volume.source && volume.source.starts_with('/'))
        {
            Some(known) if volume.mount.len() < known.mount.len() => *known = volume,
            Some(_) => {}
            None => volumes.push(volume),
        }
    }
    volumes.sort_by(|a, b| a.mount.cmp(&b.mount));
    volumes
}

/// Turns successive answers into devices and readings; rates use the host's own clock.
pub(crate) struct Sampler {
    info: HostInfo,
    previous: Option<Raw>,
}

impl Sampler {
    pub fn new(info: HostInfo) -> Self {
        Self {
            info,
            previous: None,
        }
    }

    pub fn batch(&mut self, raw: Raw, started_at: Instant, at: Instant) -> SampleBatch {
        let previous = self.previous.replace(raw.clone());
        let previous = previous.as_ref();
        let elapsed = previous.map_or(0.0, |p| raw.uptime - p.uptime);
        let mut batch = SampleBatch {
            complete_inventory: true,
            started_at,
            finished_at: at,
            ..Default::default()
        };
        let mut sample = |device: &str, metric: &str, unit, value| {
            batch
                .samples
                .push(MetricSample::new(device, metric, unit, value, at));
        };
        let rate = |now: u64, before: Option<u64>| match before {
            Some(before) if elapsed > 0.0 && now >= before => {
                SampleValue::Gauge((now - before) as f64 / elapsed)
            }
            _ => SampleValue::WarmingUp,
        };
        let share = |part: u64, before_part: u64, now: &CpuTimes, before: &CpuTimes| {
            let total = now.total.saturating_sub(before.total);
            if total == 0 || now.total < before.total || part < before_part {
                SampleValue::WarmingUp
            } else {
                SampleValue::Gauge(((part - before_part) as f64 / total as f64 * 100.0).min(100.0))
            }
        };
        let usage = |now: &CpuTimes, before: Option<&CpuTimes>| match before {
            Some(before) => (
                share(now.busy, before.busy, now, before),
                share(now.iowait, before.iowait, now, before),
            ),
            None => (SampleValue::WarmingUp, SampleValue::WarmingUp),
        };

        let (cpu, iowait) = usage(&raw.cpu, previous.map(|p| &p.cpu));
        sample("cpu", "usage", Unit::Percent, cpu);
        sample("cpu", "iowait", Unit::Percent, iowait);
        for (index, times) in &raw.cores {
            let before = previous.and_then(|p| p.cores.iter().find(|(i, _)| i == index));
            let id = format!("core:{index}");
            sample(
                &id,
                "usage",
                Unit::Percent,
                usage(times, before.map(|(_, t)| t)).0,
            );
        }
        if let Some((total, available, swap_total, swap_free)) = raw.memory {
            let used = total - available;
            for (metric, value) in [
                ("total", total),
                ("available", available),
                ("used", used),
                ("swap_total", swap_total),
                ("swap_used", swap_total.saturating_sub(swap_free)),
            ] {
                sample(
                    "memory",
                    metric,
                    Unit::Bytes,
                    SampleValue::Gauge(value as f64),
                );
            }
            sample(
                "memory",
                "usage",
                Unit::Percent,
                if total > 0 {
                    SampleValue::Gauge(used as f64 / total as f64 * 100.0)
                } else {
                    SampleValue::Failed(Reason::NoMemoryTotal)
                },
            );
            if swap_total > 0 {
                let swap_used = swap_total.saturating_sub(swap_free);
                sample(
                    "memory",
                    "swap_usage",
                    Unit::Percent,
                    SampleValue::Gauge(swap_used as f64 / swap_total as f64 * 100.0),
                );
            }
        }
        sample(
            "system",
            "uptime",
            Unit::Seconds,
            SampleValue::Gauge(raw.uptime),
        );
        if let Some(load) = raw.load {
            for (metric, value) in ["load1", "load5", "load15"].into_iter().zip(load) {
                sample("system", metric, Unit::Count, SampleValue::Gauge(value));
            }
        }
        if let Some(tasks) = raw.tasks {
            sample(
                "system",
                "processes",
                Unit::Count,
                SampleValue::Gauge(tasks as f64),
            );
        }
        for net in &raw.nets {
            let before = previous.and_then(|p| p.nets.iter().find(|n| n.name == net.name));
            let id = format!("net:{}", net.name);
            sample(
                &id,
                "receive",
                Unit::BytesPerSecond,
                rate(net.received, before.map(|n| n.received)),
            );
            sample(
                &id,
                "send",
                Unit::BytesPerSecond,
                rate(net.sent, before.map(|n| n.sent)),
            );
            sample(
                &id,
                "received",
                Unit::Bytes,
                SampleValue::Gauge(net.received as f64),
            );
            sample(
                &id,
                "sent",
                Unit::Bytes,
                SampleValue::Gauge(net.sent as f64),
            );
        }
        for (name, read, written) in &raw.disks {
            let before = previous.and_then(|p| p.disks.iter().find(|(n, ..)| n == name));
            let id = format!("disk:{name}");
            let bytes = |sectors: u64| sectors.saturating_mul(512);
            sample(
                &id,
                "read",
                Unit::BytesPerSecond,
                rate(bytes(*read), before.map(|d| bytes(d.1))),
            );
            sample(
                &id,
                "write",
                Unit::BytesPerSecond,
                rate(bytes(*written), before.map(|d| bytes(d.2))),
            );
        }
        for volume in &raw.volumes {
            let id = format!("volume:{}", volume.mount);
            let capacity = volume.used + volume.available;
            sample(
                &id,
                "total",
                Unit::Bytes,
                SampleValue::Gauge(volume.total as f64),
            );
            sample(
                &id,
                "available",
                Unit::Bytes,
                SampleValue::Gauge(volume.available as f64),
            );
            sample(
                &id,
                "usage",
                Unit::Percent,
                if capacity > 0 {
                    SampleValue::Gauge(volume.used as f64 / capacity as f64 * 100.0)
                } else {
                    SampleValue::Failed(Reason::VolumeUnavailable)
                },
            );
        }

        let info = &self.info;
        let devices = &mut batch.devices;
        devices.push(
            DeviceDescriptor::new("system", &info.hostname, DeviceKind::System)
                .detail("os", &info.os)
                .detail("kernel", &info.kernel)
                .detail("architecture", &info.arch),
        );
        let cores = info.cores.map_or(raw.cores.len(), |n| n as usize);
        devices.push(
            DeviceDescriptor::new("cpu", &info.model, DeviceKind::Cpu)
                .detail("logical_cores", cores.to_string()),
        );
        for (index, _) in &raw.cores {
            devices.push(DeviceDescriptor::new(
                format!("core:{index}"),
                "",
                DeviceKind::Core,
            ));
        }
        if raw.memory.is_some() {
            devices.push(DeviceDescriptor::new("memory", "", DeviceKind::Memory));
        }
        for net in &raw.nets {
            let mut device =
                DeviceDescriptor::new(format!("net:{}", net.name), &net.name, DeviceKind::Network)
                    .detail(
                        "interface_type",
                        if net.physical { "physical" } else { "virtual" },
                    );
            if net.default_route {
                device = device.detail("default_route", "yes");
            }
            devices.push(device);
        }
        for (name, ..) in &raw.disks {
            devices.push(DeviceDescriptor::new(
                format!("disk:{name}"),
                name,
                DeviceKind::Disk,
            ));
        }
        for volume in &raw.volumes {
            let mut device = DeviceDescriptor::new(
                format!("volume:{}", volume.mount),
                &volume.mount,
                DeviceKind::Volume,
            )
            .detail("device", &volume.source);
            if !volume.file_system.is_empty() {
                device = device.detail("file_system", &volume.file_system);
            }
            devices.push(device);
        }
        batch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const INFO: &str = "@@ info
hostname web-01
kernel Linux 6.8.0-45-generic
arch x86_64
cores 4
@@ os-release
NAME=\"Ubuntu\"
VERSION=\"24.04.1 LTS (Noble Numbat)\"
PRETTY_NAME=\"Ubuntu 24.04.1 LTS\"
@@ cpuinfo
model name\t: AMD EPYC 7B13
model name\t: AMD EPYC 7B13
";

    fn sample(uptime: &str, cpu: &str, rx: u64, read: u64) -> String {
        format!(
            "@@ uptime
{uptime} 1000.00
@@ loadavg
0.52 0.61 0.70 2/312 4242
@@ stat
cpu  {cpu} 0 0 0
cpu0 {cpu} 0 0 0
intr 1 2 3
@@ meminfo
MemTotal:        8000000 kB
MemFree:         1000000 kB
MemAvailable:    6000000 kB
SwapTotal:       2000000 kB
SwapFree:        1500000 kB
@@ net/dev
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 999 1 0 0 0 0 0 0 999 1 0 0 0 0 0 0
  eth0: {rx} 10 0 0 0 0 0 0 5000 10 0 0 0 0 0 0
docker0:0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
vethab12: 1 1 0 0 0 0 0 0 1 1 0 0 0 0 0 0
@@ net/route
Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT
docker0\t000011AC\t00000000\t0001\t0\t0\t0\t0000FFFF\t0\t0\t0
eth0\t00000000\t0101A8C0\t0003\t0\t0\t100\t00000000\t0\t0\t0
@@ diskstats
   8       0 sda 100 0 {read} 50 20 0 400 10 0 0 0
   8       1 sda1 100 0 {read} 50 20 0 400 10 0 0 0
   7       0 loop0 1 0 8 0 0 0 0 0 0 0 0
@@ block
loop0
sda
@@ physical
eth0
@@ df
Filesystem     Type     1024-blocks      Used Available Capacity Mounted on
/dev/sda1      ext4       100000000  40000000  55000000      43% /
tmpfs          tmpfs        1000000         0   1000000       0% /run
rootfs         rootfs       1000000         0   1000000       0% /init
/dev/loop0     iso9660       672000    672000         0     100% /mnt/cdrom
/dev/sda1      ext4       100000000  40000000  55000000      43% /var/snap/x
/dev/sdb1      xfs         50000000  10000000  40000000      20% /srv/my data
"
        )
    }

    #[test]
    fn info_names_the_host_system_and_processor() {
        let info = parse_info(INFO);
        assert_eq!(info.hostname, "web-01");
        assert_eq!(info.os, "Ubuntu 24.04.1 LTS");
        assert_eq!(info.kernel, "Linux 6.8.0-45-generic");
        assert_eq!(info.arch, "x86_64");
        assert_eq!(info.cores, Some(4));
        assert_eq!(info.model, "AMD EPYC 7B13");
    }

    #[test]
    fn sample_reads_counters_and_local_file_systems() {
        let raw = parse_sample(&sample("100.00", "100 0 100 700 100 0 0 0", 1000, 8)).unwrap();
        assert_eq!(raw.uptime, 100.0);
        assert_eq!(raw.load, Some([0.52, 0.61, 0.70]));
        assert_eq!(raw.tasks, Some(312));
        assert_eq!(raw.cpu.busy, 200);
        assert_eq!(raw.cpu.total, 1000);
        assert_eq!(
            raw.memory,
            Some((8_192_000_000, 6_144_000_000, 2_048_000_000, 1_536_000_000))
        );
        let names: Vec<_> = raw.nets.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["eth0", "docker0"]);
        assert!(raw.nets[0].physical && raw.nets[0].default_route);
        assert!(!raw.nets[1].physical && !raw.nets[1].default_route);
        assert_eq!(raw.disks, [("sda".to_string(), 8, 400)]);
        let mounts: Vec<_> = raw.volumes.iter().map(|v| v.mount.as_str()).collect();
        assert_eq!(mounts, ["/", "/srv/my data"]);
        assert_eq!(raw.volumes[0].file_system, "ext4");
        assert_eq!(raw.volumes[1].total, 50_000_000 * 1024);
        assert_eq!(parse_sample("@@ uptime\n@@ stat\n"), Err(Reason::NotLinux));
    }

    #[test]
    fn rates_use_the_host_clock_and_start_warming_up() {
        let mut sampler = Sampler::new(parse_info(INFO));
        let at = Instant::now();
        let first = parse_sample(&sample("100.00", "100 0 100 700 100 0 0 0", 1000, 8)).unwrap();
        let batch = sampler.batch(first, at, at);
        let value = |batch: &SampleBatch, device: &str, metric: &str| {
            batch
                .samples
                .iter()
                .find(|s| s.descriptor.key == MetricKey::new(device, metric))
                .map(|s| s.value.clone())
        };
        assert!(matches!(
            value(&batch, "cpu", "usage"),
            Some(SampleValue::WarmingUp)
        ));
        assert!(matches!(
            value(&batch, "net:eth0", "receive"),
            Some(SampleValue::WarmingUp)
        ));
        assert!(
            matches!(value(&batch, "memory", "usage"), Some(SampleValue::Gauge(v)) if v == 25.0)
        );
        assert!(
            matches!(value(&batch, "volume:/", "usage"), Some(SampleValue::Gauge(v)) if (v - 40.0 / 95.0 * 100.0).abs() < 1e-9)
        );
        assert!(
            matches!(value(&batch, "system", "load5"), Some(SampleValue::Gauge(v)) if v == 0.61)
        );
        let system = batch.devices.iter().find(|d| d.id == "system").unwrap();
        assert_eq!(system.name, "web-01");
        assert!(
            system
                .details
                .contains(&("os".into(), "Ubuntu 24.04.1 LTS".into()))
        );
        let eth0 = batch.devices.iter().find(|d| d.id == "net:eth0").unwrap();
        assert!(
            eth0.details
                .contains(&("default_route".into(), "yes".into()))
        );

        // Two seconds later by the host's clock: 600 of 1000 ticks busy, 200 waiting.
        let second =
            parse_sample(&sample("102.00", "400 0 400 900 300 0 0 0", 5000, 4104)).unwrap();
        let later = at + Duration::from_secs(5);
        let batch = sampler.batch(second, later, later);
        assert!(matches!(value(&batch, "cpu", "usage"), Some(SampleValue::Gauge(v)) if v == 60.0));
        assert!(matches!(value(&batch, "cpu", "iowait"), Some(SampleValue::Gauge(v)) if v == 20.0));
        assert!(
            matches!(value(&batch, "net:eth0", "receive"), Some(SampleValue::Gauge(v)) if v == 2000.0)
        );
        assert!(
            matches!(value(&batch, "disk:sda", "read"), Some(SampleValue::Gauge(v)) if v == 1_048_576.0)
        );
        assert!(batch.samples.iter().all(|s| s.at == later));

        // A counter that goes back, as after a driver reload, warms up again.
        let third = parse_sample(&sample("103.00", "400 0 400 900 300 0 0 0", 10, 4104)).unwrap();
        let batch = sampler.batch(third, later, later);
        assert!(matches!(
            value(&batch, "net:eth0", "receive"),
            Some(SampleValue::WarmingUp)
        ));
    }

    #[test]
    fn program_avoids_reading_commands_as_its_own_text() {
        // The ready marker is printed by the loop condition, after the whole loop is read.
        let condition = SCRIPT.lines().find(|l| l.starts_with("while ")).unwrap();
        assert!(condition.contains("echo \"@@ $r\""));
        assert!(SCRIPT.trim_end().ends_with("done"));
        assert_eq!(format!("@@ {}", "ready"), READY);
        assert_eq!(format!("@@ {}", "end"), END);
    }
}
