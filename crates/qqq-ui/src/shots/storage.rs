//! Bounded screenshot work: file operations keep their order ahead of optional thumbnails.

use super::*;
use crate::{components::thumbnail, i18n};
use image::ImageDecoder;
use std::{
    collections::VecDeque,
    fs, io,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Instant, UNIX_EPOCH},
};

const TASKS: usize = 16;
const RESULTS: usize = 8;
/// Shared pixel capacity for saves and reads, so every accepted RGBA8 screenshot fits
/// back into the library when the budget is available.
const IMAGE_BUDGET: usize = 256 * 1024 * 1024;

pub(super) enum Job {
    List(usize),
    Save(Arc<Picture>, usize),
    Prune(usize),
    Delete(PathBuf),
    Load(PathBuf),
}

pub(super) enum Done {
    Listed(Vec<ShotFile>),
    Added(ShotFile),
    Thumbnail(PathBuf, egui::ColorImage),
    Removed(Vec<PathBuf>),
    Loaded(Picture, Reservation),
    Failed(String),
}

/// Accounts for pixels while a task or result retains them.
pub(super) struct Reservation(Arc<AtomicUsize>, usize);
impl Drop for Reservation {
    fn drop(&mut self) {
        self.0.fetch_sub(self.1, Ordering::AcqRel);
    }
}
impl Reservation {
    fn shrink(&mut self, bytes: usize) {
        self.0.fetch_sub(self.1 - bytes, Ordering::AcqRel);
        self.1 = bytes;
    }
}
fn reserve(used: &Arc<AtomicUsize>, bytes: usize) -> Option<Reservation> {
    used.try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
        n.checked_add(bytes).filter(|total| *total <= IMAGE_BUDGET)
    })
    .ok()
    .map(|_| Reservation(Arc::clone(used), bytes))
}
struct Work {
    job: Job,
    _reservation: Option<Reservation>,
}
enum Task {
    Job(Work),
    Thumbnail(PathBuf),
}
#[derive(Default)]
struct Pending {
    jobs: VecDeque<Work>,
    thumbnails: VecDeque<PathBuf>,
    wanted: Vec<PathBuf>,
    active_thumbnail: Option<PathBuf>,
    closing: bool,
}
struct Shared {
    pending: Mutex<Pending>,
    ready: Condvar,
    used: Arc<AtomicUsize>,
    error: Mutex<Option<String>>,
    context: egui::Context,
}
impl Shared {
    fn next(&self) -> Option<Task> {
        let mut pending = self.pending.lock().unwrap();
        loop {
            if let Some(job) = pending.jobs.pop_front() {
                return Some(Task::Job(job));
            }
            if pending.closing {
                return None;
            }
            if let Some(path) = pending.thumbnails.pop_front() {
                pending.active_thumbnail = Some(path.clone());
                return Some(Task::Thumbnail(path));
            }
            pending = self.ready.wait(pending).unwrap();
        }
    }
    fn fail(&self, error: &str) {
        *self.error.lock().unwrap() = Some(error.into());
        self.context.request_repaint();
    }
    fn close(&self) {
        let mut pending = self.pending.lock().unwrap();
        pending.closing = true;
        pending.thumbnails.clear();
        pending.wanted.clear();
        self.ready.notify_one();
    }
}

pub(super) struct Store {
    shared: Arc<Shared>,
    done: mpsc::Receiver<Done>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Store {
    pub(super) fn new(ctx: &egui::Context, directory: PathBuf, local_time: LocalTime) -> Self {
        let shared = Arc::new(Shared {
            pending: Mutex::new(Pending::default()),
            ready: Condvar::new(),
            used: Arc::new(AtomicUsize::new(0)),
            error: Mutex::new(None),
            context: ctx.clone(),
        });
        let (outbox, done) = mpsc::sync_channel(RESULTS);
        let state = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("screenshots".into())
            .spawn(move || {
                let worker = Worker {
                    directory,
                    local_time,
                    outbox,
                    context: state.context.clone(),
                    budget: Arc::clone(&state.used),
                };
                while let Some(task) = state.next() {
                    match task {
                        Task::Job(work) => worker.run(work.job),
                        Task::Thumbnail(path) => {
                            if let Ok((picture, _reservation)) = read(&path, &state.used) {
                                let image = thumbnail(&picture, THUMBNAIL);
                                let wanted = state.pending.lock().unwrap().wanted.contains(&path);
                                if wanted
                                    && worker.outbox.try_send(Done::Thumbnail(path, image)).is_ok()
                                {
                                    worker.context.request_repaint();
                                }
                            }
                            state.pending.lock().unwrap().active_thumbnail = None;
                        }
                    }
                }
            })
            .ok();
        if worker.is_none() {
            shared.fail(i18n::t().shot_storage_unavailable);
            shared.close();
        }
        Self {
            shared,
            done,
            worker,
        }
    }
    pub(super) fn send(&self, job: Job) {
        let mut pending = self.shared.pending.lock().unwrap();
        if pending.closing || pending.jobs.len() >= TASKS {
            drop(pending);
            self.shared.fail(i18n::t().shot_busy);
            return;
        }
        let reservation = if let Job::Save(picture, _) = &job {
            let Some(reservation) = reserve(&self.shared.used, picture.rgba.len()) else {
                drop(pending);
                self.shared.fail(i18n::t().shot_memory_full);
                return;
            };
            Some(reservation)
        } else {
            None
        };
        let work = Work {
            job,
            _reservation: reservation,
        };
        pending.jobs.push_back(work);
        self.shared.ready.notify_one();
    }
    pub(super) fn thumbnails(&self, paths: Vec<PathBuf>) {
        let mut pending = self.shared.pending.lock().unwrap();
        if pending.closing {
            return;
        }
        pending.wanted = paths;
        pending.thumbnails = pending
            .wanted
            .iter()
            .filter(|path| pending.active_thumbnail.as_ref() != Some(path))
            .cloned()
            .collect();
        self.shared.ready.notify_one();
    }
    pub(super) fn take(&self) -> Vec<Done> {
        self.done.try_iter().take(RESULTS).collect()
    }
    pub(super) fn error(&self) -> Option<String> {
        self.shared.error.lock().unwrap().take()
    }
    pub(super) fn finish(&mut self, limit: Duration) {
        self.shared.close();
        let deadline = Instant::now() + limit;
        while self.worker.as_ref().is_some_and(|w| !w.is_finished()) && Instant::now() < deadline {
            // A closing page consumes metadata/results so the writer can finish accepted saves.
            self.done.try_iter().for_each(drop);
            thread::sleep(Duration::from_millis(10));
        }
        self.done.try_iter().for_each(drop);
        if let Some(worker) = self.worker.take_if(|w| w.is_finished()) {
            let _ = worker.join();
        }
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        self.shared.close();
    }
}

struct Worker {
    directory: PathBuf,
    outbox: mpsc::SyncSender<Done>,
    local_time: LocalTime,
    context: egui::Context,
    budget: Arc<AtomicUsize>,
}

impl Worker {
    fn run(&self, job: Job) {
        match job {
            Job::List(keep) => {
                let mut files = self.files();
                self.remove(files.split_off(keep.min(files.len())));
                self.send(Done::Listed(files));
            }
            Job::Save(picture, keep) => match self.save(&picture) {
                Ok(file) => {
                    self.send(Done::Added(file));

                    self.prune(keep);
                }
                Err(error) => self.send(Done::Failed(error.to_string())),
            },
            Job::Prune(keep) => self.prune(keep),
            Job::Delete(path) => match fs::remove_file(&path) {
                Ok(()) => self.send(Done::Removed(vec![path])),
                Err(error) => self.send(Done::Failed(error.to_string())),
            },
            Job::Load(path) => match read(&path, &self.budget) {
                Ok((picture, reservation)) => self.send(Done::Loaded(picture, reservation)),
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
                let (width, height) = limited_reader(&entry.path()).ok()?.into_dimensions().ok()?;
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

/// Reads one image with a size cap, accounting for decoded pixels before allocation.
fn read(path: &Path, used: &Arc<AtomicUsize>) -> io::Result<(Picture, Reservation)> {
    // The decoder supplies both dimensions and pixels from the same open file. Its
    // metadata allocation limit is active before PNG headers are interpreted.
    let decoder = limited_reader(path)?
        .into_decoder()
        .map_err(io::Error::other)?;
    let (width, height) = decoder.dimensions();
    let bytes = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .filter(|bytes| *bytes <= IMAGE_BUDGET)
        .ok_or_else(|| io::Error::other(i18n::t().shot_too_large))?;
    let decoded = usize::try_from(decoder.total_bytes())
        .ok()
        .filter(|n| *n <= IMAGE_BUDGET)
        .ok_or_else(|| io::Error::other(i18n::t().shot_too_large))?;
    let peak = if decoder.color_type() == image::ColorType::Rgba8 {
        bytes
    } else {
        bytes + decoded
    };
    let mut reservation =
        reserve(used, peak).ok_or_else(|| io::Error::other(i18n::t().shot_memory_full))?;
    let image = image::DynamicImage::from_decoder(decoder)
        .map_err(io::Error::other)?
        .into_rgba8();
    reservation.shrink(bytes);
    Ok((
        Picture {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        },
        reservation,
    ))
}

fn limited_reader(path: &Path) -> io::Result<image::ImageReader<io::BufReader<fs::File>>> {
    let mut reader = image::ImageReader::open(path)?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(IMAGE_BUDGET as u64);
    reader.limits(limits);
    Ok(reader)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paused_store() -> Store {
        let (_, done) = mpsc::sync_channel(RESULTS);
        Store {
            shared: Arc::new(Shared {
                pending: Mutex::new(Pending::default()),
                ready: Condvar::new(),
                used: Arc::new(AtomicUsize::new(0)),
                error: Mutex::new(None),
                context: egui::Context::default(),
            }),
            done,
            worker: None,
        }
    }

    #[test]
    fn file_operations_keep_their_order_ahead_of_replaceable_thumbnails() {
        let store = paused_store();
        store.thumbnails(vec!["old.png".into()]);
        store.thumbnails(vec!["visible.png".into()]);
        store.send(Job::Load("copy.png".into()));
        store.send(Job::Save(
            Arc::new(Picture {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            }),
            10,
        ));
        assert!(matches!(
            store.shared.next(),
            Some(Task::Job(Work {
                job: Job::Load(_),
                ..
            }))
        ));
        let Some(Task::Job(work)) = store.shared.next() else {
            panic!("the save follows the earlier copy")
        };
        assert!(matches!(work.job, Job::Save(..)));
        assert_eq!(store.shared.used.load(Ordering::Acquire), 4);
        drop(work);
        assert_eq!(store.shared.used.load(Ordering::Acquire), 0);
        assert!(
            matches!(store.shared.next(), Some(Task::Thumbnail(path)) if path == Path::new("visible.png"))
        );
        store.shared.close();
        assert!(store.shared.next().is_none());
    }

    #[test]
    fn every_requested_thumbnail_is_read() {
        let store = paused_store();
        let paths: Vec<PathBuf> = (0..24).map(|n| format!("shot {n}.png").into()).collect();
        store.thumbnails(paths.clone());
        let read: Vec<_> = paths
            .iter()
            .map(|_| match store.shared.next() {
                Some(Task::Thumbnail(path)) => path,
                _ => panic!("each visible tile queues a thumbnail read"),
            })
            .collect();
        assert_eq!(read, paths);
        store.shared.close();
        assert!(store.shared.next().is_none());
    }

    #[test]
    fn queue_capacity_and_pixel_budget_reject_more_work_without_blocking() {
        let store = paused_store();
        let held = reserve(&store.shared.used, IMAGE_BUDGET).unwrap();
        store.send(Job::Save(
            Arc::new(Picture {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            }),
            10,
        ));
        assert!(store.error().is_some());
        assert!(store.shared.pending.lock().unwrap().jobs.is_empty());
        drop(held);
        for _ in 0..TASKS {
            store.send(Job::Load("copy.png".into()));
        }
        store.send(Job::Load("excess.png".into()));
        assert!(store.error().is_some());
        assert_eq!(store.shared.pending.lock().unwrap().jobs.len(), TASKS);
    }

    #[test]
    fn reads_reserve_conversion_memory_and_release_it_after_use() {
        let directory =
            std::env::temp_dir().join(format!("qqq-shots-read-budget-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let used = Arc::new(AtomicUsize::new(0));
        let path = directory.join("pixels.png");
        for (color, decoded, peak) in [
            (image::ColorType::Rgba8, 16, 16),
            (image::ColorType::Rgb8, 12, 28),
        ] {
            image::save_buffer_with_format(
                &path,
                &vec![100; decoded],
                2,
                2,
                color,
                image::ImageFormat::Png,
            )
            .unwrap();
            let occupied = IMAGE_BUDGET - peak + 1;
            let held = reserve(&used, occupied).unwrap();
            assert!(read(&path, &used).is_err());
            assert_eq!(used.load(Ordering::Acquire), occupied);
            drop(held);

            let held = reserve(&used, IMAGE_BUDGET - peak).unwrap();
            let (picture, reservation) = read(&path, &used).unwrap();
            assert_eq!((picture.width, picture.height), (2, 2));
            assert_eq!(picture.rgba.len(), 16);
            assert_eq!(used.load(Ordering::Acquire), IMAGE_BUDGET - peak + 16);
            drop(held);
            assert_eq!(used.load(Ordering::Acquire), 16);
            drop(picture);
            drop(reservation);
            assert_eq!(used.load(Ordering::Acquire), 0);
        }
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn finish_drains_a_full_result_channel_and_saves_every_accepted_picture() {
        let directory =
            std::env::temp_dir().join(format!("qqq-shots-finish-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let mut store = Store::new(&egui::Context::default(), directory.clone(), |_| None);
        for _ in 0..RESULTS + 4 {
            store.send(Job::Save(
                Arc::new(Picture {
                    width: 2,
                    height: 2,
                    rgba: vec![200; 16],
                }),
                100,
            ));
        }
        assert_eq!(store.error(), None);
        // With no consumer, eight Added results fill the channel. The ninth completed
        // PNG establishes that the writer has reached the full-channel boundary.
        let deadline = Instant::now() + Duration::from_secs(5);
        let saved_files = || {
            fs::read_dir(&directory).map_or(0, |files| {
                files
                    .flatten()
                    .filter(|file| file.path().extension().is_some_and(|ext| ext == "png"))
                    .count()
            })
        };
        while saved_files() < RESULTS + 1 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(saved_files(), RESULTS + 1);
        store.finish(Duration::from_secs(5));
        assert!(store.worker.is_none());
        assert_eq!(fs::read_dir(&directory).unwrap().count(), RESULTS + 4);
        assert_eq!(store.shared.used.load(Ordering::Acquire), 0);
        fs::remove_dir_all(&directory).unwrap();
    }
}
