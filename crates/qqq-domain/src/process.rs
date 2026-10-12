//! Processes of Windows and of running WSL distributions, and the actions the GUI can
//! take on them.

use crate::Reason;

/// Where a process runs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProcessHost {
    Windows,
    /// A running WSL distribution, by the name WSL lists it under.
    Wsl(String),
}

/// One process instance. The start stamp tells it apart from a later process that reuses
/// its PID: the creation time in 100 ns units on Windows, the start time in clock ticks
/// after boot under WSL.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId {
    pub host: ProcessHost,
    pub pid: u32,
    pub start: u64,
}

/// Scheduling priority in the classes Windows uses. WSL processes map their nice value
/// onto these classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    Idle,
    BelowNormal,
    Normal,
    AboveNormal,
    High,
    Realtime,
}

impl Priority {
    /// Classes a process can be moved to, highest first.
    pub const SETTABLE: [Self; 5] = [
        Self::High,
        Self::AboveNormal,
        Self::Normal,
        Self::BelowNormal,
        Self::Idle,
    ];

    /// Nice value a WSL process gets in this class.
    pub fn nice(self) -> i32 {
        match self {
            Self::Idle => 19,
            Self::BelowNormal => 10,
            Self::Normal => 0,
            Self::AboveNormal => -5,
            Self::High => -10,
            Self::Realtime => -20,
        }
    }

    /// Class of a nice value; the nice value of each settable class maps back to it.
    pub fn from_nice(nice: i32) -> Self {
        match nice {
            15.. => Self::Idle,
            5..=14 => Self::BelowNormal,
            -2..=4 => Self::Normal,
            -7..=-3 => Self::AboveNormal,
            ..=-8 => Self::High,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub id: ProcessId,
    pub parent: Option<u32>,
    pub name: String,
    /// Command line, or the executable path when the command line is unreadable; empty
    /// when neither can be read.
    pub command: String,
    /// Account name; empty when the owner cannot be read.
    pub user: String,
    /// Processor time used since the process started, in seconds.
    pub cpu_time: f64,
    /// Private working set on Windows, resident set size under WSL, in bytes.
    pub memory: u64,
    pub threads: u32,
    pub priority: Option<Priority>,
}

/// Processes of one host at one moment.
#[derive(Clone, Debug)]
pub struct ProcessSnapshot {
    /// Host clock in seconds, measuring the time between two snapshots.
    pub clock: f64,
    /// Logical processors of the host; CPU usage is a share of all of them.
    pub processors: u32,
    pub processes: Vec<ProcessInfo>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessAction {
    /// Ends the process: TerminateProcess on Windows, SIGTERM under WSL.
    End,
    /// Ends a WSL process with SIGKILL; on Windows the same as `End`.
    Kill,
    SetPriority(Priority),
}

/// Lists the processes of one host; created and used on the polling thread.
pub trait ProcessSource {
    fn snapshot(&mut self) -> Result<ProcessSnapshot, Reason>;
}

/// Process access of the platform.
pub trait ProcessPlatform: Send + Sync {
    /// Hosts that can be listed now: Windows and every running WSL distribution.
    fn hosts(&self) -> Vec<ProcessHost>;
    fn source(&self, host: &ProcessHost) -> Box<dyn ProcessSource>;
    /// Carries out `action` once `id` still names the same running instance.
    fn act(&self, id: &ProcessId, action: ProcessAction) -> Result<(), Reason>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settable_priorities_survive_a_nice_round_trip() {
        for priority in Priority::SETTABLE {
            assert_eq!(Priority::from_nice(priority.nice()), priority);
        }
        assert_eq!(Priority::from_nice(-20), Priority::High);
        assert_eq!(Priority::from_nice(1), Priority::Normal);
    }
}
