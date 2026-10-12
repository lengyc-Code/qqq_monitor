//! Processes of running WSL distributions, read and controlled through `wsl.exe`. Every
//! request first confirms that the distribution still runs, so polling never starts a
//! stopped one. A poll pipes a POSIX shell script into the distribution that prints
//! `/proc` and `/etc/passwd`; busybox-based distributions run it as well.

use qqq_domain::*;
use std::{
    collections::HashMap,
    io::{Read, Write},
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Prints a header line `H ticks-per-second page-size processors uptime shell-pid`,
/// `N uid name` per account, and per process `P uid stat-line` and `C pid command-line`.
const LIST: &str = r#"cd /proc || exit 1
read -r up _ < uptime
echo "H $(getconf CLK_TCK 2>/dev/null || echo 100) $(getconf PAGESIZE 2>/dev/null || echo 4096) $(nproc 2>/dev/null || echo 1) $up $$"
while IFS=: read -r n _ i _; do echo "N $i $n"; done < /etc/passwd
for p in [0-9]*; do
  read -r s < "$p/stat" || continue
  u=
  while read -r k v _; do [ "$k" = Uid: ] && { u=$v; break; }; done < "$p/status"
  echo "P $u $s"
  printf 'C %s ' "$p"; tr '\0' ' ' < "$p/cmdline"; echo
done
"#;
/// Arguments: PID, start time in clock ticks, then `TERM`, `KILL` or a nice value. Exits
/// with 3 when the PID no longer names that instance.
const ACT: &str = r#"pid=$1 start=$2 action=$3
read -r s < "/proc/$pid/stat" || exit 3
set -- ${s##*") "}
[ "${20}" = "$start" ] || exit 3
case $action in
  TERM|KILL) kill -s "$action" "$pid" ;;
  *) renice "$action" -p "$pid" > /dev/null ;;
esac
"#;
/// Exit status of `ACT` for a process that is gone.
const GONE: i32 = 3;
const TIMEOUT: Duration = Duration::from_secs(10);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Runs `wsl.exe` with `input` on its standard input; returns the exit code and output.
fn wsl(args: &[&str], input: &str) -> Result<(i32, Vec<u8>), Reason> {
    let spawn_failed = |error: std::io::Error| Reason::CallFailed {
        api: "wsl.exe",
        code: error.raw_os_error().unwrap_or(0) as u32,
    };
    let mut child = Command::new("wsl.exe")
        .args(args)
        .env("WSL_UTF8", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(spawn_failed)?;
    if let Some(mut stdin) = child.stdin.take() {
        // A command that exits early closes the pipe; its exit code tells why.
        let _ = stdin.write_all(input.as_bytes());
    }
    let mut stdout = child.stdout.take();
    let reader = thread::spawn(move || {
        let mut output = Vec::new();
        if let Some(stdout) = &mut stdout {
            let _ = stdout.read_to_end(&mut output);
        }
        output
    });
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        match child.try_wait().map_err(spawn_failed)? {
            Some(status) => break Some(status),
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    };
    let output = reader.join().unwrap_or_default();
    match status {
        Some(status) => Ok((status.code().unwrap_or(-1), output)),
        None => Err(Reason::CallFailed {
            api: "wsl.exe",
            code: 0x5B4, // ERROR_TIMEOUT
        }),
    }
}

/// Text that `wsl.exe` printed itself: UTF-8 with `WSL_UTF8`, UTF-16 on older versions.
fn decode(output: &[u8]) -> String {
    if output.contains(&0) {
        let units: Vec<u16> = output
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(output).into_owned()
    }
}

/// Distributions WSL reports as running, in its order; empty without WSL.
pub fn running() -> Vec<String> {
    match wsl(&["--list", "--running", "--quiet"], "") {
        Ok((0, output)) => decode(&output)
            .lines()
            .map(|line| line.trim_matches(|c: char| c.is_whitespace() || c == '\0'))
            .filter(|line| !line.is_empty())
            .map(String::from)
            .collect(),
        _ => Vec::new(),
    }
}

fn ensure_running(distro: &str) -> Result<(), Reason> {
    if running().iter().any(|d| d == distro) {
        Ok(())
    } else {
        Err(Reason::HostStopped)
    }
}

pub struct WslProcesses {
    distro: String,
}

impl WslProcesses {
    pub fn new(distro: &str) -> Self {
        Self {
            distro: distro.into(),
        }
    }
}

impl ProcessSource for WslProcesses {
    fn snapshot(&mut self) -> Result<ProcessSnapshot, Reason> {
        ensure_running(&self.distro)?;
        let (code, output) = wsl(&["-d", &self.distro, "--cd", "/", "-e", "sh", "-s"], LIST)?;
        if code != 0 {
            return Err(Reason::CallFailed {
                api: "wsl.exe",
                code: code as u32,
            });
        }
        parse(&self.distro, &String::from_utf8_lossy(&output))
            .ok_or(Reason::BadData { api: "wsl.exe" })
    }
}

/// Fields of a `/proc/PID/stat` line.
struct Stat<'a> {
    pid: u32,
    comm: &'a str,
    /// Fields after the command name, starting with the state (field 3).
    fields: Vec<&'a str>,
}

impl<'a> Stat<'a> {
    fn parse(line: &'a str) -> Option<Self> {
        let open = line.find(" (")?;
        let close = line.rfind(") ")?;
        Some(Self {
            pid: line[..open].parse().ok()?,
            comm: line.get(open + 2..close)?,
            fields: line[close + 2..].split_whitespace().collect(),
        })
    }
    /// Field `n` as numbered in proc(5).
    fn field<T: std::str::FromStr>(&self, n: usize) -> Option<T> {
        self.fields.get(n.checked_sub(3)?)?.parse().ok()
    }
}

fn parse(distro: &str, output: &str) -> Option<ProcessSnapshot> {
    let mut header = None;
    let mut users = HashMap::new();
    let mut stats = Vec::new();
    let mut commands = HashMap::new();
    for line in output.lines() {
        let Some((tag, rest)) = line.split_once(' ') else {
            continue;
        };
        match tag {
            "H" => header = Some(rest.split_whitespace().collect::<Vec<_>>()),
            "N" => {
                if let Some((uid, name)) = rest.split_once(' ') {
                    users.entry(uid).or_insert(name);
                }
            }
            "P" => {
                if let Some((uid, stat)) = rest.split_once(' ')
                    && let Some(stat) = Stat::parse(stat)
                {
                    stats.push((uid, stat));
                }
            }
            "C" => {
                if let Some((pid, command)) = rest.split_once(' ')
                    && let Ok(pid) = pid.parse::<u32>()
                {
                    commands.insert(pid, command.trim_end());
                }
            }
            _ => {}
        }
    }
    let header = header?;
    let number = |i: usize| header.get(i).and_then(|v| v.parse::<f64>().ok());
    let (ticks, page, processors, clock, shell) =
        (number(0)?, number(1)?, number(2)?, number(3)?, number(4)?);
    // The script runs as `sh` under the Relay and SessionLeader processes of its own WSL
    // session; none of them belongs to the distribution.
    let parents: HashMap<u32, (u32, &str)> = stats
        .iter()
        .filter_map(|(_, s)| Some((s.pid, (s.field(4)?, s.comm))))
        .collect();
    let mut own = vec![shell as u32];
    while let Some(&(parent, _)) = parents.get(own.last()?)
        && let Some(&(_, comm)) = parents.get(&parent)
        && (comm.starts_with("Relay(") || comm == "SessionLeader")
    {
        own.push(parent);
    }
    let processes = stats
        .into_iter()
        .filter(|(_, s)| !own.contains(&s.pid))
        .filter_map(|(uid, s)| {
            let command = commands.get(&s.pid).copied().unwrap_or_default();
            // The kernel truncates the command name to 15 bytes; the program name in the
            // command line restores it.
            let program = command
                .split(' ')
                .next()
                .and_then(|first| first.rsplit('/').next())
                .unwrap_or_default();
            let name = if program.starts_with(s.comm) {
                program
            } else {
                s.comm
            };
            let cpu_ticks = s.field::<u64>(14)? + s.field::<u64>(15)?;
            Some(ProcessInfo {
                id: ProcessId {
                    host: ProcessHost::Wsl(distro.into()),
                    pid: s.pid,
                    start: s.field(22)?,
                },
                parent: s.field::<u32>(4).filter(|p| *p != 0),
                name: name.into(),
                command: command.into(),
                user: users.get(uid).copied().unwrap_or(uid).into(),
                cpu_time: cpu_ticks as f64 / ticks,
                memory: s.field::<u64>(24)?.saturating_mul(page as u64),
                threads: s.field(20)?,
                priority: Some(Priority::from_nice(s.field(19)?)),
            })
        })
        .collect();
    Some(ProcessSnapshot {
        clock,
        processors: processors as u32,
        processes,
    })
}

/// Ends or renices a process as root, once its start time confirms the instance.
pub fn act(distro: &str, pid: u32, start: u64, action: ProcessAction) -> Result<(), Reason> {
    ensure_running(distro)?;
    let (argument, api) = match action {
        ProcessAction::End => ("TERM".to_string(), "kill"),
        ProcessAction::Kill => ("KILL".to_string(), "kill"),
        ProcessAction::SetPriority(priority) => (priority.nice().to_string(), "renice"),
    };
    let (pid, start) = (pid.to_string(), start.to_string());
    let args = [
        "-d", distro, "-u", "root", "--cd", "/", "-e", "sh", "-s", &pid, &start, &argument,
    ];
    match wsl(&args, ACT)?.0 {
        0 => Ok(()),
        GONE => Err(Reason::ProcessGone),
        code => Err(Reason::CallFailed {
            api,
            code: code as u32,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTPUT: &str = "H 100 4096 32 19952.56 40953
N 0 root
N 1000 lengy
P 0 1 (systemd) S 0 1 1 0 -1 4194560 8982 1093739 113 8943 51 49 32400 731 20 0 1 0 16212 22355968 3233 18446744073709551615
C 1 /sbin/init
P 1000 996 (systemd-timesyn) S 1 139 139 0 -1 4194560 1033 0 5 0 8 21 0 0 20 -5 2 0 16257 93220864 1621 18446744073709551615
C 996 /lib/systemd/systemd-timesyncd
P 0 2 (init-systemd(Ub) S 1 0 0 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 16200 0 100 0
C 2 /init
P 0 40951 (SessionLeader) S 2 0 0 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 99000 0 100 0
C 40951 /init
P 0 40952 (Relay(40953)) S 40951 0 0 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 99001 0 100 0
C 40952 /init
P 1000 40953 (sh) S 40952 0 0 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 99002 0 100 0
C 40953 sh -s
";

    #[test]
    fn parses_processes_and_leaves_out_its_own_session() {
        let snapshot = parse("Ubuntu", OUTPUT).unwrap();
        assert_eq!(snapshot.processors, 32);
        assert_eq!(snapshot.clock, 19952.56);
        let pids: Vec<_> = snapshot.processes.iter().map(|p| p.id.pid).collect();
        assert_eq!(pids, [1, 996, 2]);
        let timesync = &snapshot.processes[1];
        assert_eq!(timesync.id.host, ProcessHost::Wsl("Ubuntu".into()));
        assert_eq!(timesync.id.start, 16257);
        assert_eq!(timesync.name, "systemd-timesyncd");
        assert_eq!(timesync.user, "lengy");
        assert_eq!(timesync.parent, Some(1));
        assert_eq!(timesync.threads, 2);
        assert_eq!(timesync.memory, 1621 * 4096);
        assert_eq!(timesync.cpu_time, 0.29);
        assert_eq!(timesync.priority, Some(Priority::AboveNormal));
        let init = &snapshot.processes[2];
        assert_eq!(init.name, "init-systemd(Ub");
        assert_eq!(init.parent, Some(1));
    }

    /// Needs a running WSL distribution: `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs a running WSL distribution"]
    fn ends_a_process_in_a_running_distribution() {
        let distro = running()
            .into_iter()
            .next()
            .expect("a running distribution");
        let mut child = Command::new("wsl.exe")
            .args(["-d", &distro, "-e", "sleep", "30"])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .unwrap();
        let mut source = WslProcesses::new(&distro);
        let deadline = Instant::now() + Duration::from_secs(10);
        let sleep = loop {
            let snapshot = source.snapshot().unwrap();
            if let Some(p) = snapshot
                .processes
                .into_iter()
                .find(|p| p.command == "sleep 30")
            {
                break p;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(200));
        };
        let (pid, start) = (sleep.id.pid, sleep.id.start);
        assert_eq!(
            act(&distro, pid, start + 1, ProcessAction::End),
            Err(Reason::ProcessGone)
        );
        act(
            &distro,
            pid,
            start,
            ProcessAction::SetPriority(Priority::Idle),
        )
        .unwrap();
        let reniced = source.snapshot().unwrap();
        let reniced = reniced.processes.iter().find(|p| p.id.pid == pid).unwrap();
        assert_eq!(reniced.priority, Some(Priority::Idle));
        act(&distro, pid, start, ProcessAction::End).unwrap();
        assert!(child.wait().unwrap().code() != Some(0));
    }

    #[test]
    fn decodes_both_list_encodings() {
        let utf16: Vec<u8> = "Ubuntu\r\n"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        assert_eq!(decode(&utf16), "Ubuntu\r\n");
        assert_eq!(decode(b"Ubuntu\n"), "Ubuntu\n");
    }
}
