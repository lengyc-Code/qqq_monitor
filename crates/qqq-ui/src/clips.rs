//! Clipboard history and its GUI projection. Preparation and content comparison run on a
//! worker; the GUI shares content and uploads the small prepared previews.

mod worker;

use eframe::egui;
use qqq_domain::Clip;
use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use worker::{Job, Pending, Request, Snapshot};

/// Most content bytes the history keeps; a single newest item can stand on its own.
pub const BUDGET: usize = 256 * 1024 * 1024;

pub struct ClipEntry {
    pub id: u64,
    pub clip: Arc<Clip>,
    pub at: Instant,
    pub thumbnail: Option<egui::TextureHandle>,
    pub chars: usize,
    pub preview: Arc<str>,
}

#[derive(Default)]
struct Shared {
    pending: Pending,
    latest: Option<Snapshot>,
    stop: bool,
}

pub struct Clips {
    pub entries: VecDeque<ClipEntry>,
    shared: Arc<(Mutex<Shared>, Condvar)>,
    worker: Option<JoinHandle<()>>,
    next: u64,
    /// Answers older than a local clear, removal or trim cannot restore those entries.
    minimum: u64,
}

impl Clips {
    pub fn new(ctx: &egui::Context) -> Self {
        let shared = Arc::new((Mutex::new(Shared::default()), Condvar::new()));
        let state = Arc::clone(&shared);
        let context = ctx.clone();
        let worker = thread::Builder::new()
            .name("clipboard-history".into())
            .spawn(move || worker::run(state, context))
            .ok();
        Self {
            entries: VecDeque::new(),
            shared,
            worker,
            next: 0,
            minimum: 0,
        }
    }

    fn send(&mut self, job: Job, local: bool) {
        self.next += 1;
        if local {
            self.minimum = self.next;
        }
        let (state, changed) = &*self.shared;
        let mut state = state.lock().unwrap();
        if self.worker.is_some() && !state.stop {
            state.pending.push(Request {
                sequence: self.next,
                job,
            });
            changed.notify_one();
        }
    }

    /// Hands content to the worker without comparing or scanning its bytes on the GUI.
    pub fn record(&mut self, _: &egui::Context, clip: impl Into<Arc<Clip>>, keep: usize) {
        self.send(
            Job::Record {
                clip: clip.into(),
                keep,
                at: Instant::now(),
            },
            false,
        );
    }

    pub fn update(&mut self, ctx: &egui::Context) {
        let latest = self.shared.0.lock().unwrap().latest.take();
        let Some(snapshot) = latest.filter(|s| s.sequence >= self.minimum) else {
            return;
        };
        let mut previous = std::mem::take(&mut self.entries);
        self.entries = snapshot
            .entries
            .into_iter()
            .map(|entry| {
                let texture = previous
                    .iter_mut()
                    .find(|old| old.id == entry.id)
                    .and_then(|old| old.thumbnail.take())
                    .or_else(|| {
                        entry.thumbnail.as_ref().map(|image| {
                            ctx.load_texture(
                                format!("clip-{}", entry.id),
                                image.as_ref().clone(),
                                egui::TextureOptions::LINEAR,
                            )
                        })
                    });
                ClipEntry {
                    id: entry.id,
                    clip: entry.clip,
                    at: entry.at,
                    thumbnail: texture,
                    chars: entry.chars,
                    preview: entry.preview,
                }
            })
            .collect();
    }

    pub fn trim(&mut self, keep: usize) {
        self.entries.truncate(keep.max(1));
        self.send(Job::Trim(keep), true);
    }

    pub fn remove(&mut self, id: u64) {
        self.entries.retain(|entry| entry.id != id);
        self.send(Job::Remove(id), true);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.send(Job::Clear, true);
    }

    pub fn finish(&mut self, limit: Duration) {
        {
            let (state, changed) = &*self.shared;
            let mut state = state.lock().unwrap();
            state.stop = true;
            state.pending = Pending::default();
            state.latest = None;
            changed.notify_one();
        }
        let deadline = Instant::now() + limit;
        while self.worker.as_ref().is_some_and(|w| !w.is_finished()) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        if let Some(worker) = self.worker.take_if(|w| w.is_finished()) {
            let _ = worker.join();
        }
    }
}

impl Drop for Clips {
    fn drop(&mut self) {
        self.finish(Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparation_shares_content_and_keeps_clear_from_old_answers() {
        let ctx = egui::Context::default();
        let mut clips = Clips::new(&ctx);
        let content = Arc::new(Clip::Text("你好\nworld".into()));
        clips.record(&ctx, Arc::clone(&content), 3);
        let deadline = Instant::now() + Duration::from_secs(3);
        while clips.entries.is_empty() && Instant::now() < deadline {
            clips.update(&ctx);
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(clips.entries.len(), 1);
        assert!(Arc::ptr_eq(&clips.entries[0].clip, &content));
        assert_eq!(clips.entries[0].chars, 8);
        assert_eq!(&*clips.entries[0].preview, "你好\nworld");
        clips.clear();
        clips.update(&ctx);
        assert!(clips.entries.is_empty());
        clips.finish(Duration::from_secs(2));
        assert!(clips.worker.is_none());
        clips.update(&ctx);
        assert!(clips.entries.is_empty());
    }
}
