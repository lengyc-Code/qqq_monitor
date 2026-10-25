//! Clipboard history: what was copied to the clipboard while the clipboard manager is on,
//! newest first, kept in memory for this run.

use crate::components::thumbnail;
use eframe::egui;
use qqq_domain::Clip;
use std::{collections::VecDeque, time::Instant};

/// Most memory the entries take together, in bytes; the oldest entries go first beyond it.
pub const BUDGET: usize = 256 * 1024 * 1024;
/// Largest picture preview, in pixels.
const THUMBNAIL: [usize; 2] = [360, 200];

pub struct ClipEntry {
    pub id: u64,
    pub clip: Clip,
    /// When the content was last copied.
    pub at: Instant,
    /// Preview of a picture.
    pub thumbnail: Option<egui::TextureHandle>,
    /// Characters of a text, counted once.
    pub chars: usize,
}

#[derive(Default)]
pub struct Clips {
    pub entries: VecDeque<ClipEntry>,
    next: u64,
}

impl Clips {
    /// Records content copied to the clipboard as the newest entry, keeping at most `keep`.
    /// Content already in the history moves to the top instead.
    pub fn record(&mut self, ctx: &egui::Context, clip: Clip, keep: usize) {
        self.next += 1;
        let thumbnail = match self.entries.iter().position(|entry| entry.clip == clip) {
            Some(index) => self.entries.remove(index).and_then(|entry| entry.thumbnail),
            None => match &clip {
                Clip::Picture(picture) => Some(ctx.load_texture(
                    format!("clip-{}", self.next),
                    thumbnail(picture, THUMBNAIL),
                    egui::TextureOptions::LINEAR,
                )),
                _ => None,
            },
        };
        let chars = match &clip {
            Clip::Text(text) => text.chars().count(),
            _ => 0,
        };
        self.entries.push_front(ClipEntry {
            id: self.next,
            clip,
            at: Instant::now(),
            thumbnail,
            chars,
        });
        self.trim(keep);
    }

    /// Drops the oldest entries beyond `keep` and beyond [`BUDGET`], keeping the newest.
    pub fn trim(&mut self, keep: usize) {
        self.trim_to(keep, BUDGET);
    }

    fn trim_to(&mut self, keep: usize, budget: usize) {
        self.entries.truncate(keep.max(1));
        let mut total: usize = self.entries.iter().map(|e| e.clip.bytes()).sum();
        while total > budget && self.entries.len() > 1 {
            if let Some(entry) = self.entries.pop_back() {
                total -= entry.clip.bytes();
            }
        }
    }

    pub fn remove(&mut self, id: u64) {
        self.entries.retain(|entry| entry.id != id);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qqq_domain::Picture;
    use std::sync::Arc;

    fn texts(clips: &Clips) -> Vec<String> {
        clips
            .entries
            .iter()
            .map(|e| match &e.clip {
                Clip::Text(text) => text.clone(),
                other => format!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn copies_move_to_the_top_and_the_oldest_go_first() {
        let ctx = egui::Context::default();
        let mut clips = Clips::default();
        for text in ["a", "b", "c", "a"] {
            clips.record(&ctx, Clip::Text(text.into()), 3);
        }
        assert_eq!(texts(&clips), ["a", "c", "b"]);
        clips.record(&ctx, Clip::Text("d".into()), 3);
        assert_eq!(texts(&clips), ["d", "a", "c"]);
        let id = clips.entries[1].id;
        clips.remove(id);
        assert_eq!(texts(&clips), ["d", "c"]);
        clips.trim(1);
        assert_eq!(texts(&clips), ["d"]);
    }

    #[test]
    fn pictures_stay_within_the_memory_budget() {
        let ctx = egui::Context::default();
        let mut clips = Clips::default();
        for shade in 0..5_u8 {
            let picture = Picture {
                width: 16,
                height: 16,
                rgba: vec![shade; 16 * 16 * 4],
            };
            clips.record(&ctx, Clip::Picture(Arc::new(picture)), 50);
        }
        assert_eq!(clips.entries.len(), 5);
        // Each picture takes 1 KiB, so four fit in 4 KiB.
        clips.trim_to(50, 4 * 1024);
        assert_eq!(clips.entries.len(), 4);
        assert!(clips.entries.iter().all(|e| e.thumbnail.is_some()));
        let Clip::Picture(newest) = &clips.entries[0].clip else {
            panic!("a picture")
        };
        assert_eq!(newest.rgba[0], 4);
    }
}
