//! Screenshot library metadata and visible thumbnail textures.
//! File operations and scheduling belong to the storage worker.

mod storage;

use eframe::egui;
use qqq_domain::Picture;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};
use storage::{Done, Job, Store};

/// Start of the names of files owned by the screenshot library.
const PREFIX: &str = "QQQ Screenshot ";
/// Largest thumbnail, in pixels.
pub const THUMBNAIL: [usize; 2] = [480, 270];

/// Local date and time as year, month, day, hour, minute, second and millisecond.
pub type LocalTime = fn(SystemTime) -> Option<[u16; 7]>;

#[derive(Clone)]
pub struct ShotFile {
    pub path: PathBuf,
    pub taken: SystemTime,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
}

pub struct Shot {
    pub file: ShotFile,
    pub thumbnail: Option<egui::TextureHandle>,
}

pub enum Event {
    Loaded(Picture),
    Failed(String),
}

pub struct Shots {
    storage: Store,
    pub directory: PathBuf,
    /// Newest first.
    pub entries: Vec<Shot>,
    pub listed: bool,
    visible: Vec<PathBuf>,
}

impl Shots {
    pub fn new(
        ctx: &egui::Context,
        directory: PathBuf,
        keep: usize,
        local_time: LocalTime,
    ) -> Self {
        let storage = Store::new(ctx, directory.clone(), local_time);
        storage.send(Job::List(keep));
        Self {
            storage,
            directory,
            entries: Vec::new(),
            listed: false,
            visible: Vec::new(),
        }
    }

    /// Queues a screenshot, or reports capacity exhaustion through [`Self::update`].
    pub fn save(&self, picture: Arc<Picture>, keep: usize) {
        self.storage.send(Job::Save(picture, keep));
    }

    pub fn prune(&self, keep: usize) {
        self.storage.send(Job::Prune(keep));
    }
    pub fn delete(&self, path: &Path) {
        self.storage.send(Job::Delete(path.into()));
    }
    pub fn load(&self, path: &Path) {
        self.storage.send(Job::Load(path.into()));
    }

    /// Retains textures for every visible tile, and replaces pending thumbnail reads. The
    /// viewport bounds how many tiles are visible. An empty slice releases the page's
    /// textures and cancels its queued reads.
    pub fn show_visible_thumbnails(&mut self, paths: &[PathBuf]) {
        self.visible = paths.to_vec();
        let mut missing = Vec::new();
        for shot in &mut self.entries {
            if self.visible.contains(&shot.file.path) {
                if shot.thumbnail.is_none() {
                    missing.push(shot.file.path.clone());
                }
            } else {
                shot.thumbnail = None;
            }
        }
        self.storage.thumbnails(missing);
    }

    #[cfg(test)]
    fn show_thumbnails(&mut self) {
        let paths: Vec<_> = self.entries.iter().map(|s| s.file.path.clone()).collect();
        self.show_visible_thumbnails(&paths);
    }

    pub fn update(&mut self, ctx: &egui::Context) -> Vec<Event> {
        let mut events = Vec::new();
        for done in self.storage.take() {
            match done {
                Done::Listed(files) => {
                    self.entries = files
                        .into_iter()
                        .map(|file| Shot {
                            file,
                            thumbnail: None,
                        })
                        .collect();
                    self.listed = true;
                }
                Done::Added(file) => {
                    self.entries.retain(|s| s.file.path != file.path);
                    self.entries.insert(
                        0,
                        Shot {
                            file,
                            thumbnail: None,
                        },
                    );
                }
                Done::Thumbnail(path, image) => {
                    if self.visible.contains(&path)
                        && let Some(shot) = self.entries.iter_mut().find(|s| s.file.path == path)
                    {
                        shot.thumbnail = Some(ctx.load_texture(
                            path.to_string_lossy(),
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }
                Done::Removed(paths) => self.entries.retain(|s| !paths.contains(&s.file.path)),
                Done::Loaded(picture, _reservation) => events.push(Event::Loaded(picture)),
                Done::Failed(error) => events.push(Event::Failed(error)),
            }
        }
        if let Some(error) = self.storage.error() {
            events.push(Event::Failed(error));
        }
        events
    }

    /// Cancels optional thumbnail work and lets accepted file operations finish.
    pub fn finish(&mut self, limit: Duration) {
        self.storage.finish(limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, thread, time::Instant};

    fn picture(width: u32, height: u32) -> Picture {
        Picture {
            width,
            height,
            rgba: (0..width * height * 4).map(|v| v as u8).collect(),
        }
    }

    fn wait(shots: &mut Shots, ctx: &egui::Context, until: impl Fn(&Shots) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !until(shots) && Instant::now() < deadline {
            shots.update(ctx);
            thread::sleep(Duration::from_millis(10));
        }
        assert!(until(shots));
    }

    #[test]
    fn listed_screenshots_get_thumbnails_once_shown() {
        let directory = std::env::temp_dir().join(format!("qqq-shots-lazy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let ctx = egui::Context::default();
        let local = |_| Some([2026, 10, 16, 9, 0, 0, 0]);
        // A screenshot an earlier run saved.
        let mut earlier = Shots::new(&ctx, directory.clone(), 5, local);
        wait(&mut earlier, &ctx, |s| s.listed);
        earlier.save(Arc::new(picture(6, 4)), 5);
        wait(&mut earlier, &ctx, |s| !s.entries.is_empty());
        earlier.finish(Duration::from_secs(5));

        let mut shots = Shots::new(&ctx, directory.clone(), 5, local);
        wait(&mut shots, &ctx, |s| s.listed && s.entries.len() == 1);
        thread::sleep(Duration::from_millis(200));
        shots.update(&ctx);
        assert!(shots.entries[0].thumbnail.is_none());
        shots.show_thumbnails();
        wait(&mut shots, &ctx, |s| s.entries[0].thumbnail.is_some());
        shots.finish(Duration::from_secs(5));
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn saved_large_screenshots_can_be_copied_and_previewed_after_reopening() {
        // Cover a large capture and the full pixel capacity accepted by the writer.
        for (width, height) in [(8192, 4320), (8192, 8192)] {
            let directory = std::env::temp_dir().join(format!(
                "qqq-shots-large-{}-{width}x{height}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&directory);
            let ctx = egui::Context::default();
            let mut rgba = vec![255; width as usize * height as usize * 4];
            rgba[..4].copy_from_slice(&[10, 20, 30, 255]);
            let last = rgba.len() - 4;
            rgba[last..].copy_from_slice(&[40, 50, 60, 255]);
            let expected = Arc::new(Picture {
                width,
                height,
                rgba,
            });
            let mut shots = Shots::new(&ctx, directory.clone(), 5, |_| None);
            wait(&mut shots, &ctx, |s| s.listed);
            shots.save(Arc::clone(&expected), 5);
            wait(&mut shots, &ctx, |s| s.entries.len() == 1);
            shots.finish(Duration::from_secs(5));
            drop(shots);

            let mut shots = Shots::new(&ctx, directory.clone(), 5, |_| None);
            wait(&mut shots, &ctx, |s| s.listed && s.entries.len() == 1);
            shots.load(&shots.entries[0].file.path.clone());
            let deadline = Instant::now() + Duration::from_secs(20);
            let loaded = 'loaded: loop {
                for event in shots.update(&ctx) {
                    match event {
                        Event::Loaded(picture) => break 'loaded picture,
                        Event::Failed(error) => panic!("copying {width}x{height}: {error}"),
                    }
                }
                assert!(
                    Instant::now() < deadline,
                    "copying {width}x{height} timed out"
                );
                thread::sleep(Duration::from_millis(10));
            };
            assert_eq!(loaded, *expected);
            drop(loaded);
            drop(expected);
            shots.show_thumbnails();
            wait(&mut shots, &ctx, |s| s.entries[0].thumbnail.is_some());
            let size = shots.entries[0].thumbnail.as_ref().unwrap().size();
            assert!(size[0] > 0 && size[0] <= THUMBNAIL[0]);
            assert!(size[1] > 0 && size[1] <= THUMBNAIL[1]);
            shots.finish(Duration::from_secs(5));
            drop(shots);
            fs::remove_dir_all(&directory).unwrap();
        }
    }

    #[test]
    fn every_visible_tile_keeps_its_thumbnail() {
        let ctx = egui::Context::default();
        let directory =
            std::env::temp_dir().join(format!("qqq-shots-visible-{}", std::process::id()));
        let mut shots = Shots::new(&ctx, directory, 5, |_| None);
        let paths: Vec<PathBuf> = (0..24).map(|n| format!("shot {n}.png").into()).collect();
        shots.entries = paths
            .iter()
            .map(|path| Shot {
                file: ShotFile {
                    path: path.clone(),
                    taken: SystemTime::UNIX_EPOCH,
                    width: 1,
                    height: 1,
                    bytes: 4,
                },
                thumbnail: Some(ctx.load_texture(
                    path.to_string_lossy(),
                    egui::ColorImage::new([1, 1], vec![egui::Color32::WHITE]),
                    egui::TextureOptions::LINEAR,
                )),
            })
            .collect();
        shots.show_visible_thumbnails(&paths);
        assert!(shots.entries.iter().all(|shot| shot.thumbnail.is_some()));
        shots.show_visible_thumbnails(&paths[..2]);
        let kept = shots
            .entries
            .iter()
            .filter(|s| s.thumbnail.is_some())
            .count();
        assert_eq!(kept, 2);
        shots.finish(Duration::from_secs(5));
    }

    #[test]
    fn the_library_keeps_the_newest_screenshots() {
        let directory = std::env::temp_dir().join(format!("qqq-shots-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("keep me.png"), b"not ours").unwrap();
        let ctx = egui::Context::default();
        let local = |_| Some([2026, 10, 15, 14, 30, 12, 0]);
        let mut shots = Shots::new(&ctx, directory.clone(), 2, local);
        wait(&mut shots, &ctx, |s| s.listed);
        assert!(shots.entries.is_empty());
        for size in [3, 4, 5] {
            shots.save(Arc::new(picture(size, 2)), 2);
            wait(&mut shots, &ctx, |s| {
                s.entries.first().is_some_and(|e| e.file.width == size)
            });
        }
        wait(&mut shots, &ctx, |s| s.entries.len() == 2);
        let names: Vec<_> = shots
            .entries
            .iter()
            .map(|s| {
                s.file
                    .path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            names,
            [
                "QQQ Screenshot 2026-10-15 143012 (3).png",
                "QQQ Screenshot 2026-10-15 143012 (2).png"
            ]
        );
        shots.load(&shots.entries[0].file.path.clone());
        let deadline = Instant::now() + Duration::from_secs(20);
        let loaded = loop {
            if let Some(Event::Loaded(p)) = shots.update(&ctx).into_iter().next() {
                break p;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(loaded, picture(5, 2));
        shots.delete(&shots.entries[1].file.path.clone());
        wait(&mut shots, &ctx, |s| s.entries.len() == 1);
        shots.finish(Duration::from_secs(5));
        let mut left: Vec<_> = fs::read_dir(&directory)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(
            left,
            ["QQQ Screenshot 2026-10-15 143012 (3).png", "keep me.png"]
        );
        fs::remove_dir_all(&directory).unwrap();
    }
}
