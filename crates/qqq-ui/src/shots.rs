//! Screenshot library: the newest screenshots as PNG files in a folder of the data
//! directory. A worker thread writes, lists, reads and removes the files, so the GUI never
//! waits for the disk.

use crate::components::thumbnail;
use eframe::egui;
use qqq_domain::Picture;
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Start of the names of the files the library keeps; other files in the folder stay
/// untouched.
const PREFIX: &str = "QQQ Screenshot ";
/// Largest thumbnail, in pixels.
pub const THUMBNAIL: [usize; 2] = [480, 270];

/// Local date and time as year, month, day, hour, minute, second and millisecond.
pub type LocalTime = fn(SystemTime) -> Option<[u16; 7]>;

/// A screenshot file.
#[derive(Clone)]
pub struct ShotFile {
    pub path: PathBuf,
    pub taken: SystemTime,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
}

/// A screenshot as the page shows it.
pub struct Shot {
    pub file: ShotFile,
    pub thumbnail: Option<egui::TextureHandle>,
}

enum Job {
    /// Lists the folder, keeping the newest `usize`.
    List(usize),
    /// Reads the thumbnails of these screenshots.
    Thumbnails(Vec<PathBuf>),
    /// Saves a screenshot, keeping the newest `usize`.
    Save(Arc<Picture>, usize),
    /// Keeps the newest `usize`.
    Prune(usize),
    Delete(PathBuf),
    /// Reads a whole screenshot.
    Load(PathBuf),
}

enum Done {
    Listed(Vec<ShotFile>),
    Added(ShotFile),
    Thumbnail(PathBuf, egui::ColorImage),
    Removed(Vec<PathBuf>),
    Loaded(Picture),
    Failed(String),
}

/// Outcome of a job that the GUI acts on.
pub enum Event {
    /// A screenshot read with [`Shots::load`].
    Loaded(Picture),
    /// A file could not be written, read or removed, with the system's description.
    Failed(String),
}

pub struct Shots {
    jobs: Option<mpsc::Sender<Job>>,
    done: mpsc::Receiver<Done>,
    worker: Option<thread::JoinHandle<()>>,
    pub directory: PathBuf,
    /// Newest first.
    pub entries: Vec<Shot>,
    /// The folder has been listed.
    pub listed: bool,
    /// The page has shown the screenshots, so listed ones get their thumbnails.
    shown: bool,
}

impl Shots {
    /// Starts the worker on `directory`, which lists the folder and keeps the newest `keep`.
    pub fn new(
        ctx: &egui::Context,
        directory: PathBuf,
        keep: usize,
        local_time: LocalTime,
    ) -> Self {
        let (jobs, inbox) = mpsc::channel();
        let (outbox, done) = mpsc::channel();
        let worker = Worker {
            directory: directory.clone(),
            outbox,
            local_time,
            context: ctx.clone(),
        };
        let worker = thread::Builder::new()
            .name("screenshots".into())
            .spawn(move || {
                for job in inbox {
                    worker.run(job);
                }
            })
            .ok();
        let shots = Self {
            jobs: Some(jobs),
            done,
            worker,
            directory,
            entries: Vec::new(),
            listed: false,
            shown: false,
        };
        shots.send(Job::List(keep));
        shots
    }

    fn send(&self, job: Job) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(job);
        }
    }

    /// Saves a new screenshot and removes the oldest beyond `keep`.
    pub fn save(&self, picture: Arc<Picture>, keep: usize) {
        self.send(Job::Save(picture, keep));
    }

    /// Removes the oldest screenshots beyond `keep`.
    pub fn prune(&self, keep: usize) {
        self.send(Job::Prune(keep));
    }

    pub fn delete(&self, path: &Path) {
        self.send(Job::Delete(path.into()));
    }

    /// Reads the thumbnails of the listed screenshots, once the page first shows them;
    /// screenshots taken meanwhile have theirs already.
    pub fn show_thumbnails(&mut self) {
        if !self.shown {
            self.shown = true;
            self.read_thumbnails();
        }
    }

    fn read_thumbnails(&self) {
        let paths: Vec<_> = self
            .entries
            .iter()
            .filter(|shot| shot.thumbnail.is_none())
            .map(|shot| shot.file.path.clone())
            .collect();
        if !paths.is_empty() {
            self.send(Job::Thumbnails(paths));
        }
    }

    /// Reads a whole screenshot; it arrives as [`Event::Loaded`].
    pub fn load(&self, path: &Path) {
        self.send(Job::Load(path.into()));
    }

    /// Applies the worker's results and returns those the GUI acts on.
    pub fn update(&mut self, ctx: &egui::Context) -> Vec<Event> {
        let mut events = Vec::new();
        while let Ok(done) = self.done.try_recv() {
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
                    if self.shown {
                        self.read_thumbnails();
                    }
                }
                Done::Added(file) => {
                    self.entries.retain(|shot| shot.file.path != file.path);
                    self.entries.insert(
                        0,
                        Shot {
                            file,
                            thumbnail: None,
                        },
                    );
                }
                Done::Thumbnail(path, image) => {
                    if let Some(shot) = self.entries.iter_mut().find(|s| s.file.path == path) {
                        shot.thumbnail = Some(ctx.load_texture(
                            path.to_string_lossy(),
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }
                Done::Removed(paths) => self.entries.retain(|s| !paths.contains(&s.file.path)),
                Done::Loaded(picture) => events.push(Event::Loaded(picture)),
                Done::Failed(error) => events.push(Event::Failed(error)),
            }
        }
        events
    }

    /// Lets the worker finish the jobs it has, waiting at most `limit`.
    pub fn finish(&mut self, limit: Duration) {
        self.jobs = None;
        let deadline = Instant::now() + limit;
        while let Some(worker) = &self.worker
            && !worker.is_finished()
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(10));
        }
        if let Some(worker) = self.worker.take_if(|w| w.is_finished()) {
            let _ = worker.join();
        }
    }
}

struct Worker {
    directory: PathBuf,
    outbox: mpsc::Sender<Done>,
    local_time: LocalTime,
    context: egui::Context,
}

impl Worker {
    fn run(&self, job: Job) {
        match job {
            Job::List(keep) => {
                let mut files = self.files();
                self.remove(files.split_off(keep.min(files.len())));
                self.send(Done::Listed(files));
            }
            Job::Thumbnails(paths) => {
                for path in paths {
                    if let Some(image) = read(&path) {
                        self.send(Done::Thumbnail(path, thumbnail(&image, THUMBNAIL)));
                    }
                }
            }
            Job::Save(picture, keep) => match self.save(&picture) {
                Ok(file) => {
                    let path = file.path.clone();
                    self.send(Done::Added(file));
                    self.send(Done::Thumbnail(path, thumbnail(&picture, THUMBNAIL)));
                    self.prune(keep);
                }
                Err(error) => self.send(Done::Failed(error.to_string())),
            },
            Job::Prune(keep) => self.prune(keep),
            Job::Delete(path) => match fs::remove_file(&path) {
                Ok(()) => self.send(Done::Removed(vec![path])),
                Err(error) => self.send(Done::Failed(error.to_string())),
            },
            Job::Load(path) => match image::open(&path) {
                Ok(image) => {
                    let image = image.into_rgba8();
                    self.send(Done::Loaded(Picture {
                        width: image.width(),
                        height: image.height(),
                        rgba: image.into_raw(),
                    }));
                }
                Err(error) => self.send(Done::Failed(error.to_string())),
            },
        }
    }

    fn send(&self, done: Done) {
        let _ = self.outbox.send(done);
        self.context.request_repaint();
    }

    /// The library's files, newest first. Leftovers of interrupted saves are removed.
    fn files(&self) -> Vec<ShotFile> {
        let Ok(entries) = fs::read_dir(&self.directory) else {
            return Vec::new();
        };
        let mut files: Vec<_> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                if !name.starts_with(PREFIX) {
                    return None;
                }
                if name.ends_with(".png.tmp") {
                    let _ = fs::remove_file(entry.path());
                    return None;
                }
                if !name.ends_with(".png") {
                    return None;
                }
                let metadata = entry.metadata().ok()?;
                let (width, height) = image::ImageReader::open(entry.path())
                    .ok()?
                    .into_dimensions()
                    .ok()?;
                Some(ShotFile {
                    path: entry.path(),
                    taken: metadata.modified().ok()?,
                    width,
                    height,
                    bytes: metadata.len(),
                })
            })
            .collect();
        files.sort_by(|a, b| b.taken.cmp(&a.taken).then_with(|| b.path.cmp(&a.path)));
        files
    }

    fn remove(&self, files: Vec<ShotFile>) {
        let removed: Vec<_> = files
            .into_iter()
            .filter(|file| fs::remove_file(&file.path).is_ok())
            .map(|file| file.path)
            .collect();
        if !removed.is_empty() {
            self.send(Done::Removed(removed));
        }
    }

    fn prune(&self, keep: usize) {
        let mut files = self.files();
        self.remove(files.split_off(keep.min(files.len())));
    }

    /// Writes the picture under a name with the local time, through a temporary file.
    fn save(&self, picture: &Picture) -> io::Result<ShotFile> {
        fs::create_dir_all(&self.directory)?;
        let now = SystemTime::now();
        let stamp = match (self.local_time)(now) {
            Some([year, month, day, hour, minute, second, _]) => {
                format!("{year:04}-{month:02}-{day:02} {hour:02}{minute:02}{second:02}")
            }
            None => now
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
                .to_string(),
        };
        let path = (1..)
            .map(|n| match n {
                1 => format!("{PREFIX}{stamp}.png"),
                _ => format!("{PREFIX}{stamp} ({n}).png"),
            })
            .map(|name| self.directory.join(name))
            .find(|path| !path.exists())
            .unwrap_or_default();
        let temporary = path.with_extension("png.tmp");
        let written = image::save_buffer_with_format(
            &temporary,
            &picture.rgba,
            picture.width,
            picture.height,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(io::Error::other)
        .and_then(|()| fs::rename(&temporary, &path));
        if let Err(error) = written {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        let metadata = fs::metadata(&path)?;
        Ok(ShotFile {
            taken: metadata.modified().unwrap_or(now),
            path,
            width: picture.width,
            height: picture.height,
            bytes: metadata.len(),
        })
    }
}

/// The picture of a PNG file.
fn read(path: &Path) -> Option<Picture> {
    let image = image::open(path).ok()?.into_rgba8();
    Some(Picture {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
