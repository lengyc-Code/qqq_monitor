//! Notes: Markdown text the user keeps, one file per note in a folder of the data
//! directory. A worker thread lists, reads, writes and removes the files, so the GUI never
//! waits for the disk; an edit is written once typing has paused.

use crate::shots::LocalTime;
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Start of the names of the files the notes keep; other files in the folder stay
/// untouched.
const PREFIX: &str = "QQQ Note ";
/// Pause after the last edit before a note is written.
pub const SAVE_DELAY: Duration = Duration::from_millis(800);
/// Longest an edit waits for a pause before it is written anyway.
const SAVE_WAIT: Duration = Duration::from_secs(5);

/// A note as the GUI holds it.
pub struct Note {
    /// Identity for this session.
    pub id: u64,
    /// Markdown text with `\n` line breaks.
    pub text: String,
    /// Last change, by the user or of the file.
    pub modified: SystemTime,
    /// File holding the note, once written.
    path: Option<PathBuf>,
    /// Times of the first and the latest edit not yet handed to the worker.
    dirty: Option<(Instant, Instant)>,
}

impl Note {
    fn new(id: u64) -> Self {
        Self {
            id,
            text: String::new(),
            modified: SystemTime::now(),
            path: None,
            dirty: None,
        }
    }

    /// First line with text, without the Markdown marks that begin it; empty for a blank
    /// note.
    pub fn title(&self) -> &str {
        self.text
            .lines()
            .map(|line| {
                let line = line
                    .trim()
                    .trim_start_matches(['#', '>', '-', '*', '+'])
                    .trim_start();
                ["[ ]", "[x]", "[X]"]
                    .iter()
                    .fold(line, |line, task| line.strip_prefix(task).unwrap_or(line))
                    .trim()
            })
            .find(|line| !line.is_empty())
            .unwrap_or_default()
    }

    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }
}

enum Job {
    List,
    /// Writes a note under its file, or a new file when it has none; a blank note removes
    /// its file instead.
    Save {
        id: u64,
        path: Option<PathBuf>,
        text: String,
    },
    Delete {
        id: u64,
        path: Option<PathBuf>,
    },
}

/// A note file the worker read.
struct Listed {
    path: PathBuf,
    modified: SystemTime,
    text: String,
}

enum Done {
    Listed(Vec<Listed>),
    Saved {
        id: u64,
        path: PathBuf,
    },
    /// The blank note's file is gone.
    Emptied {
        id: u64,
    },
    Failed(String),
    /// A deleted note's file is still there.
    DeleteFailed(String),
}

/// Why the latest write or deletion of a note's file failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoteError {
    Save(String),
    Delete(String),
}

impl NoteError {
    /// The system's description of the failure.
    pub fn text(&self) -> &str {
        match self {
            Self::Save(text) | Self::Delete(text) => text,
        }
    }
}

pub struct Notes {
    jobs: Option<mpsc::Sender<Job>>,
    done: mpsc::Receiver<Done>,
    worker: Option<thread::JoinHandle<()>>,
    pub directory: PathBuf,
    /// Most recently changed first.
    pub entries: Vec<Note>,
    /// The folder has been listed.
    pub listed: bool,
    /// The note the window shows.
    pub current: u64,
    next_id: u64,
    /// Writes handed to the worker and not answered yet.
    writing: usize,
    /// Why the latest write or deletion failed, until a write succeeds.
    pub error: Option<NoteError>,
}

impl Notes {
    /// Starts the worker on `directory`, which lists the folder; `wake` repaints whoever
    /// shows the notes when the worker answers.
    pub fn new(
        directory: PathBuf,
        local_time: LocalTime,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let (jobs, inbox) = mpsc::channel();
        let (outbox, done) = mpsc::channel();
        let mut worker = Worker {
            directory: directory.clone(),
            outbox,
            local_time,
            wake: Box::new(wake),
            paths: HashMap::new(),
        };
        let worker = thread::Builder::new()
            .name("notes".into())
            .spawn(move || {
                for job in inbox {
                    worker.run(job);
                }
            })
            .ok();
        let mut notes = Self {
            jobs: Some(jobs),
            done,
            worker,
            directory,
            entries: Vec::new(),
            listed: false,
            current: 0,
            next_id: 0,
            writing: 0,
            error: None,
        };
        notes.send(Job::List);
        notes.current = notes.add();
        notes
    }

    fn send(&self, job: Job) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(job);
        }
    }

    /// A new blank note at the top of the list.
    fn add(&mut self) -> u64 {
        self.next_id += 1;
        self.entries.insert(0, Note::new(self.next_id));
        self.next_id
    }

    pub fn get(&self, id: u64) -> Option<&Note> {
        self.entries.iter().find(|note| note.id == id)
    }

    /// The note the window shows.
    pub fn current(&self) -> &Note {
        self.get(self.current)
            .or(self.entries.first())
            .expect("the notes always hold the current note")
    }

    /// The note the window shows, for editing; follow a change with [`Self::edited`].
    pub fn current_mut(&mut self) -> &mut Note {
        let id = self.current;
        let index = self
            .entries
            .iter()
            .position(|note| note.id == id)
            .unwrap_or(0);
        &mut self.entries[index]
    }

    /// Records that the current note's text changed: it moves to the top and is written
    /// once typing pauses.
    pub fn edited(&mut self) {
        let id = self.current;
        if let Some(index) = self.entries.iter().position(|note| note.id == id) {
            let mut note = self.entries.remove(index);
            note.modified = SystemTime::now();
            let now = Instant::now();
            note.dirty = Some((note.dirty.map_or(now, |(first, _)| first), now));
            self.entries.insert(0, note);
        }
    }

    /// Shows another note, writing the one left at once and dropping it when blank.
    pub fn select(&mut self, id: u64) {
        if id == self.current || self.get(id).is_none() {
            return;
        }
        self.leave();
        self.current = id;
    }

    /// Shows a new blank note, or the current one when it is still blank.
    pub fn create(&mut self) {
        if self.current().is_blank() {
            return;
        }
        self.leave();
        self.current = self.add();
    }

    /// Removes a note and its file. The window then shows the newest other note, or a new
    /// blank one.
    pub fn delete(&mut self, id: u64) {
        let Some(index) = self.entries.iter().position(|note| note.id == id) else {
            return;
        };
        let note = self.entries.remove(index);
        self.send(Job::Delete {
            id,
            path: note.path,
        });
        if id == self.current {
            self.current = match self.entries.first() {
                Some(note) => note.id,
                None => self.add(),
            };
        }
    }

    /// Writes the current note now and drops it when blank, unless it is the only note.
    fn leave(&mut self) {
        self.write(self.current);
        if self.current().is_blank() && self.entries.len() > 1 {
            let id = self.current;
            self.entries.retain(|note| note.id != id);
        }
    }

    /// Hands a note with unwritten edits to the worker.
    fn write(&mut self, id: u64) {
        let Some(note) = self.entries.iter_mut().find(|note| note.id == id) else {
            return;
        };
        if note.dirty.take().is_none() {
            return;
        }
        let job = Job::Save {
            id,
            path: note.path.clone(),
            text: note.text.clone(),
        };
        self.writing += 1;
        self.send(job);
    }

    /// Writes the notes whose typing has paused, or with `all` every unwritten note.
    /// Returns how long until the next pending write is due.
    pub fn flush(&mut self, all: bool) -> Option<Duration> {
        let mut next = None::<Duration>;
        let due: Vec<_> = self
            .entries
            .iter()
            .filter_map(|note| {
                let (first, last) = note.dirty?;
                let left = SAVE_DELAY
                    .saturating_sub(last.elapsed())
                    .min(SAVE_WAIT.saturating_sub(first.elapsed()));
                if all || left.is_zero() {
                    return Some(note.id);
                }
                next = Some(next.map_or(left, |n| n.min(left)));
                None
            })
            .collect();
        for id in due {
            self.write(id);
        }
        next
    }

    /// Whether every edit has been written.
    pub fn saved(&self) -> bool {
        self.writing == 0 && self.entries.iter().all(|note| note.dirty.is_none())
    }

    /// Applies the worker's answers.
    pub fn update(&mut self) {
        while let Ok(done) = self.done.try_recv() {
            match done {
                Done::Listed(files) => {
                    // Notes started before the listing arrived stay on top.
                    for file in files {
                        self.next_id += 1;
                        self.entries.push(Note {
                            id: self.next_id,
                            text: file.text,
                            modified: file.modified,
                            path: Some(file.path),
                            dirty: None,
                        });
                    }
                    self.listed = true;
                    // The blank note made at start gives way to the newest listed note.
                    let first = self.entries.first().map(|note| note.id);
                    if first == Some(self.current)
                        && self.current().is_blank()
                        && self.current().path.is_none()
                        && self.current().dirty.is_none()
                        && self.entries.len() > 1
                    {
                        self.entries.remove(0);
                        self.current = self.entries[0].id;
                    }
                }
                Done::Saved { id, path } => {
                    self.writing = self.writing.saturating_sub(1);
                    self.error = None;
                    if let Some(note) = self.entries.iter_mut().find(|note| note.id == id) {
                        note.path = Some(path);
                    }
                }
                Done::Emptied { id } => {
                    self.writing = self.writing.saturating_sub(1);
                    if let Some(note) = self.entries.iter_mut().find(|note| note.id == id) {
                        note.path = None;
                    }
                }
                Done::Failed(error) => {
                    self.writing = self.writing.saturating_sub(1);
                    self.error = Some(NoteError::Save(error));
                }
                // A deletion is no write, so the writes under way stay counted.
                Done::DeleteFailed(error) => self.error = Some(NoteError::Delete(error)),
            }
        }
    }

    /// Writes every unwritten note and lets the worker finish, waiting at most `limit`.
    pub fn finish(&mut self, limit: Duration) {
        self.flush(true);
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
    wake: Box<dyn Fn() + Send>,
    /// Files this worker created, by note; a write that set out before the GUI learnt the
    /// name still finds the file.
    paths: HashMap<u64, PathBuf>,
}

impl Worker {
    fn run(&mut self, job: Job) {
        match job {
            Job::List => {
                let files = self.files();
                self.send(Done::Listed(files));
            }
            Job::Save { id, path, text } => {
                let path = self.paths.get(&id).cloned().or(path);
                let done = if text.trim().is_empty() {
                    self.paths.remove(&id);
                    match path.map(fs::remove_file) {
                        Some(Err(error)) if error.kind() != io::ErrorKind::NotFound => {
                            Done::Failed(error.to_string())
                        }
                        _ => Done::Emptied { id },
                    }
                } else {
                    match self.save(path, &text) {
                        Ok(path) => {
                            self.paths.insert(id, path.clone());
                            Done::Saved { id, path }
                        }
                        Err(error) => Done::Failed(error.to_string()),
                    }
                };
                self.send(done);
            }
            Job::Delete { id, path } => {
                if let Some(path) = self.paths.remove(&id).or(path)
                    && let Err(error) = fs::remove_file(&path)
                    && error.kind() != io::ErrorKind::NotFound
                {
                    self.send(Done::DeleteFailed(error.to_string()));
                }
            }
        }
    }

    fn send(&self, done: Done) {
        let _ = self.outbox.send(done);
        (self.wake)();
    }

    /// The notes' files, most recently changed first. Leftovers of interrupted writes are
    /// removed.
    fn files(&self) -> Vec<Listed> {
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
                if name.ends_with(".md.tmp") {
                    let _ = fs::remove_file(entry.path());
                    return None;
                }
                if !name.ends_with(".md") {
                    return None;
                }
                let modified = entry.metadata().ok()?.modified().ok()?;
                let text = read(&entry.path()).ok()?;
                Some(Listed {
                    path: entry.path(),
                    modified,
                    text,
                })
            })
            .collect();
        files.sort_by(|a, b| {
            b.modified
                .cmp(&a.modified)
                .then_with(|| b.path.cmp(&a.path))
        });
        files
    }

    /// Writes the text through a temporary file, under a new name with the local time
    /// when the note has no file yet.
    fn save(&self, path: Option<PathBuf>, text: &str) -> io::Result<PathBuf> {
        fs::create_dir_all(&self.directory)?;
        let path = match path {
            Some(path) => path,
            None => self.new_path(),
        };
        let temporary = path.with_extension("md.tmp");
        let written = fs::write(&temporary, text).and_then(|()| fs::rename(&temporary, &path));
        if let Err(error) = written {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        Ok(path)
    }

    fn new_path(&self) -> PathBuf {
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
        (1..)
            .map(|n| match n {
                1 => format!("{PREFIX}{stamp}.md"),
                _ => format!("{PREFIX}{stamp} ({n}).md"),
            })
            .map(|name| self.directory.join(name))
            .find(|path| !path.exists() && !self.paths.values().any(|p| p == path))
            .unwrap_or_default()
    }
}

/// Text of a note file with `\n` line breaks; a byte order mark and invalid UTF-8 are
/// tolerated.
fn read(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    Ok(String::from_utf8_lossy(bytes).replace("\r\n", "\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait(notes: &mut Notes, until: impl Fn(&Notes) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !until(notes) && Instant::now() < deadline {
            notes.update();
            thread::sleep(Duration::from_millis(10));
        }
        assert!(until(notes));
    }

    fn names(directory: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(directory)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    fn type_text(notes: &mut Notes, text: &str) {
        notes.current_mut().text = text.into();
        notes.edited();
    }

    #[test]
    fn titles_skip_markdown_marks_and_blank_lines() {
        let mut note = Note::new(1);
        assert_eq!(note.title(), "");
        note.text = "\n  \n## Shopping list\n- milk".into();
        assert_eq!(note.title(), "Shopping list");
        note.text = "- [ ] call back".into();
        assert_eq!(note.title(), "call back");
        note.text = "> 引用".into();
        assert_eq!(note.title(), "引用");
    }

    #[test]
    fn a_failed_deletion_leaves_the_writes_under_way_counted() {
        let directory =
            std::env::temp_dir().join(format!("qqq-notes-failed-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let mut notes = Notes::new(directory.clone(), |_| None, || {});
        let (answers, done) = mpsc::channel();
        notes.done = done;
        notes.writing = 1;
        answers.send(Done::DeleteFailed("denied".into())).unwrap();
        notes.update();
        assert_eq!(notes.writing, 1);
        assert!(!notes.saved());
        assert_eq!(notes.error, Some(NoteError::Delete("denied".into())));
        answers.send(Done::Failed("full".into())).unwrap();
        notes.update();
        assert_eq!(notes.writing, 0);
        assert_eq!(notes.error, Some(NoteError::Save("full".into())));
        notes.finish(Duration::from_secs(5));
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn notes_are_written_listed_and_removed() {
        let directory = std::env::temp_dir().join(format!("qqq-notes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("keep me.md"), "not ours").unwrap();
        fs::write(
            directory.join("QQQ Note 2026-10-01 090000.md"),
            "\u{feff}# Old\r\nline",
        )
        .unwrap();
        fs::write(
            directory.join("QQQ Note 2026-10-01 090000.md.tmp"),
            "left over",
        )
        .unwrap();
        let local = |_| Some([2026, 10, 23, 14, 30, 12, 0]);
        let mut notes = Notes::new(directory.clone(), local, || {});
        wait(&mut notes, |n| n.listed);
        // The blank start note gives way to the listed one.
        assert_eq!(notes.entries.len(), 1);
        assert_eq!(notes.current().text, "# Old\nline");
        assert_eq!(notes.current().title(), "Old");

        // A new note gets a file once written, and keeps it on later writes.
        notes.create();
        let first = notes.current;
        type_text(&mut notes, "first");
        notes.flush(true);
        type_text(&mut notes, "first, longer");
        notes.flush(true);
        wait(&mut notes, |n| n.saved());
        assert_eq!(
            names(&directory),
            [
                "QQQ Note 2026-10-01 090000.md",
                "QQQ Note 2026-10-23 143012.md",
                "keep me.md"
            ]
        );
        assert_eq!(
            fs::read_to_string(directory.join("QQQ Note 2026-10-23 143012.md")).unwrap(),
            "first, longer"
        );

        // A second note in the same second takes the next free name.
        notes.create();
        type_text(&mut notes, "second");
        assert!(notes.flush(false).is_some());
        notes.flush(true);
        wait(&mut notes, |n| n.saved());
        assert!(names(&directory).contains(&"QQQ Note 2026-10-23 143012 (2).md".to_owned()));
        assert_eq!(notes.entries[0].id, notes.current);

        // Creating again on a blank note keeps it; leaving a blank note drops it.
        notes.create();
        let blank = notes.current;
        notes.create();
        assert_eq!(notes.current, blank);
        notes.select(first);
        assert!(notes.get(blank).is_none());

        // Emptying a note removes its file; deleting removes the note and its file.
        type_text(&mut notes, "  ");
        notes.flush(true);
        wait(&mut notes, |n| n.saved());
        assert!(!directory.join("QQQ Note 2026-10-23 143012.md").exists());
        let second = notes
            .entries
            .iter()
            .find(|n| n.text == "second")
            .unwrap()
            .id;
        notes.delete(second);
        notes.finish(Duration::from_secs(5));
        assert_eq!(
            names(&directory),
            ["QQQ Note 2026-10-01 090000.md", "keep me.md"]
        );
        fs::remove_dir_all(&directory).unwrap();
    }
}
