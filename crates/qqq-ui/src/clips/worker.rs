use super::{BUDGET, Shared};
use crate::components::thumbnail;
use eframe::egui;
use qqq_domain::Clip;
use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex},
    time::Instant,
};

const THUMBNAIL: [usize; 2] = [360, 200];
const PENDING_RECORDS: usize = 32;
const TEXT_LINES: usize = 3;
const LINE_CHARS: usize = 200;

pub(super) enum Job {
    Record {
        clip: Arc<Clip>,
        keep: usize,
        at: Instant,
    },
    Trim(usize),
    Remove(u64),
    Clear,
}

pub(super) struct Request {
    pub sequence: u64,
    pub job: Job,
}

#[derive(Default)]
pub(super) struct Pending {
    jobs: VecDeque<Request>,
    bytes: usize,
}

impl Pending {
    pub fn push(&mut self, request: Request) {
        match &request.job {
            Job::Clear => {
                self.jobs.clear();
                self.bytes = 0;
            }
            Job::Trim(keep) => {
                if let Some(Request {
                    sequence,
                    job: Job::Trim(previous),
                }) = self.jobs.back_mut()
                {
                    *previous = (*previous).min(*keep);
                    *sequence = request.sequence;
                    return;
                }
            }
            Job::Remove(id) => {
                if let Some(Request {
                    sequence,
                    job: Job::Remove(previous),
                }) = self.jobs.back_mut()
                    && previous == id
                {
                    *sequence = request.sequence;
                    return;
                }
            }
            Job::Record { clip, .. } => {
                let bytes = clip.bytes();
                while self.bytes.saturating_add(bytes) > BUDGET
                    || self
                        .jobs
                        .iter()
                        .filter(|r| matches!(r.job, Job::Record { .. }))
                        .count()
                        >= PENDING_RECORDS
                {
                    let Some(index) = self
                        .jobs
                        .iter()
                        .position(|r| matches!(r.job, Job::Record { .. }))
                    else {
                        break;
                    };
                    if let Some(Request {
                        job: Job::Record { clip, .. },
                        ..
                    }) = self.jobs.remove(index)
                    {
                        self.bytes -= clip.bytes();
                    }
                }
                self.bytes = self.bytes.saturating_add(bytes);
            }
        }
        self.jobs.push_back(request);
    }

    fn pop(&mut self) -> Option<Request> {
        let request = self.jobs.pop_front()?;
        if let Job::Record { clip, .. } = &request.job {
            self.bytes -= clip.bytes();
        }
        Some(request)
    }
}

#[derive(Clone)]
pub(super) struct Prepared {
    pub id: u64,
    pub clip: Arc<Clip>,
    pub at: Instant,
    pub chars: usize,
    pub preview: Arc<str>,
    pub thumbnail: Option<Arc<egui::ColorImage>>,
}

pub(super) struct Snapshot {
    pub sequence: u64,
    pub entries: VecDeque<Prepared>,
}

#[derive(Default)]
struct History {
    entries: VecDeque<Prepared>,
    next: u64,
}

impl History {
    fn apply(&mut self, job: Job) {
        match job {
            Job::Record { clip, keep, at } => {
                let existing = self
                    .entries
                    .iter()
                    .position(|entry| Arc::ptr_eq(&entry.clip, &clip) || *entry.clip == *clip)
                    .and_then(|index| self.entries.remove(index));
                let mut entry = existing.unwrap_or_else(|| {
                    self.next += 1;
                    let (chars, preview) = match &*clip {
                        Clip::Text(text) => (text.chars().count(), Arc::from(preview(text))),
                        _ => (0, Arc::from("")),
                    };
                    let thumbnail = match &*clip {
                        Clip::Picture(picture) => Some(Arc::new(thumbnail(picture, THUMBNAIL))),
                        _ => None,
                    };
                    Prepared {
                        id: self.next,
                        clip,
                        at,
                        chars,
                        preview,
                        thumbnail,
                    }
                });
                entry.at = at;
                self.entries.push_front(entry);
                self.trim(keep, BUDGET);
            }
            Job::Trim(keep) => self.trim(keep, BUDGET),
            Job::Remove(id) => self.entries.retain(|entry| entry.id != id),
            Job::Clear => self.entries.clear(),
        }
    }

    fn trim(&mut self, keep: usize, budget: usize) {
        self.entries.truncate(keep.max(1));
        let mut bytes: usize = self.entries.iter().map(|entry| entry.clip.bytes()).sum();
        while bytes > budget && self.entries.len() > 1 {
            if let Some(entry) = self.entries.pop_back() {
                bytes -= entry.clip.bytes();
            }
        }
    }
}

pub(super) fn run(shared: Arc<(Mutex<Shared>, Condvar)>, context: egui::Context) {
    let (state, changed) = &*shared;
    let mut history = History::default();
    loop {
        let request = {
            let mut state = state.lock().unwrap();
            loop {
                if state.stop {
                    return;
                }
                if let Some(request) = state.pending.pop() {
                    break request;
                }
                state = changed.wait(state).unwrap();
            }
        };
        history.apply(request.job);
        let mut state = state.lock().unwrap();
        if state.stop {
            return;
        }
        state.latest = Some(Snapshot {
            sequence: request.sequence,
            entries: history.entries.clone(),
        });
        drop(state);
        context.request_repaint();
    }
}

/// A bounded text preview, prepared together with the full character count.
fn preview(text: &str) -> String {
    let mut lines: Vec<String> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(TEXT_LINES + 1)
        .map(|line| {
            let mut shown: String = line.chars().take(LINE_CHARS).collect();
            if shown.len() < line.len() {
                shown.push('…');
            }
            shown
        })
        .collect();
    if lines.len() > TEXT_LINES {
        lines.truncate(TEXT_LINES);
        if let Some(last) = lines.last_mut() {
            last.push_str(" …");
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use qqq_domain::Picture;

    fn record(text: &str, keep: usize) -> Job {
        Job::Record {
            clip: Arc::new(Clip::Text(text.into())),
            keep,
            at: Instant::now(),
        }
    }

    #[test]
    fn preparation_deduplicates_moves_and_bounds_history() {
        let mut history = History::default();
        for text in ["a", "b", "c", "a"] {
            history.apply(record(text, 3));
        }
        let text = |h: &History| {
            h.entries
                .iter()
                .map(|e| e.preview.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(text(&history), ["a", "c", "b"]);
        history.apply(record("d", 3));
        assert_eq!(text(&history), ["d", "a", "c"]);
        let id = history.entries[1].id;
        history.apply(Job::Remove(id));
        assert_eq!(text(&history), ["d", "c"]);
        history.apply(Job::Trim(1));
        assert_eq!(text(&history), ["d"]);
        assert_eq!(preview("1\n2\n3\n4"), "1\n2\n3 …");
        assert_eq!(preview(&"界".repeat(205)), format!("{}…", "界".repeat(200)));
    }

    #[test]
    fn prepared_pictures_and_pending_records_have_memory_bounds() {
        let mut history = History::default();
        for shade in 0..5 {
            history.apply(Job::Record {
                clip: Arc::new(Clip::Picture(Arc::new(Picture {
                    width: 16,
                    height: 16,
                    rgba: vec![shade; 1024],
                }))),
                keep: 50,
                at: Instant::now(),
            });
        }
        history.trim(50, 4096);
        assert_eq!(history.entries.len(), 4);
        assert!(history.entries.iter().all(|e| e.thumbnail.is_some()));
        let mut pending = Pending::default();
        for sequence in 1..=100 {
            pending.push(Request {
                sequence,
                job: record("text", 50),
            });
        }
        assert_eq!(pending.jobs.len(), PENDING_RECORDS);
        assert_eq!(pending.jobs.back().unwrap().sequence, 100);
        pending.push(Request {
            sequence: 101,
            job: Job::Clear,
        });
        assert_eq!(pending.bytes, 0);
        assert_eq!(pending.jobs.len(), 1);
    }

    #[test]
    fn a_larger_limit_does_not_restore_trimmed_entries_or_trim_later_copies() {
        let mut history = History::default();
        for text in ["a", "b", "c"] {
            history.apply(record(text, 3));
        }
        let mut pending = Pending::default();
        pending.push(Request {
            sequence: 1,
            job: Job::Trim(1),
        });
        pending.push(Request {
            sequence: 2,
            job: Job::Trim(3),
        });
        while let Some(request) = pending.pop() {
            history.apply(request.job);
        }
        assert_eq!(history.entries.len(), 1);
        assert_eq!(&*history.entries[0].preview, "c");
        pending.push(Request {
            sequence: 3,
            job: Job::Trim(1),
        });
        pending.push(Request {
            sequence: 4,
            job: record("d", 3),
        });
        pending.push(Request {
            sequence: 5,
            job: Job::Trim(3),
        });
        while let Some(request) = pending.pop() {
            history.apply(request.job);
        }
        assert_eq!(history.entries.len(), 2);
        assert_eq!(&*history.entries[0].preview, "d");
        assert_eq!(&*history.entries[1].preview, "c");
    }
}
