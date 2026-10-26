//! Notes: Markdown text the user keeps, one file per note in a folder of the data
//! directory. A worker thread lists, reads, writes and removes the files, so the GUI never
//! waits for the disk; an edit is written once typing has paused.

mod storage;

use crate::{i18n, shots::LocalTime};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime},
};
use storage::Worker;

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
    /// First unsubmitted edit and latest edit awaiting a successful write.
    dirty: Option<(Instant, Instant)>,
    /// Latest edit handed to the worker, for distinguishing later edits from its answer.
    submitted: Option<Instant>,
}

impl Note {
    fn new(id: u64) -> Self {
        Self {
            id,
            text: String::new(),
            modified: SystemTime::now(),
            path: None,
            dirty: None,
            submitted: None,
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
        at: Instant,
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
        at: Instant,
    },
    /// The blank note's file is gone.
    Emptied {
        id: u64,
        at: Instant,
    },
    Failed {
        id: u64,
        at: Instant,
        error: String,
    },
    /// A deleted note's file is still there.
    DeleteFailed(String),
}

/// A failed write of a note, kept until a write of the same or a later edit succeeds.
struct Failure {
    /// Latest edit the failed write carried.
    at: Instant,
    /// When the failure arrived among all failures, for showing the latest one.
    order: u64,
    error: String,
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
    /// Why the latest write or deletion still standing failed.
    pub error: Option<NoteError>,
    /// Failed writes by note.
    save_errors: BTreeMap<u64, Failure>,
    /// Latest failed deletion with its arrival order, until a write succeeds.
    delete_error: Option<(u64, String)>,
    /// Failures recorded so far; orders the save and delete errors.
    failures: u64,
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
            save_errors: BTreeMap::new(),
            delete_error: None,
            failures: 0,
        };
        notes.send(Job::List);
        notes.current = notes.add();
        notes
    }

    fn send(&self, job: Job) -> bool {
        self.jobs
            .as_ref()
            .is_some_and(|jobs| jobs.send(job).is_ok())
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
            let first = note.dirty.map_or(now, |(first, _)| {
                if note.submitted.is_some_and(|at| at >= first)
                    || self
                        .save_errors
                        .get(&id)
                        .is_some_and(|failure| failure.at >= first)
                {
                    now
                } else {
                    first
                }
            });
            note.dirty = Some((first, now));
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
        self.save_errors.remove(&id);
        self.refresh_error();
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
        if self.current().is_blank() && self.current().dirty.is_none() && self.entries.len() > 1 {
            let id = self.current;
            self.entries.retain(|note| note.id != id);
        }
    }

    /// Hands a note with unwritten edits to the worker.
    fn write(&mut self, id: u64) {
        let Some(note) = self.entries.iter_mut().find(|note| note.id == id) else {
            return;
        };
        let Some((_, at)) = note.dirty else { return };
        if note.submitted.is_some_and(|sent| sent >= at) {
            return;
        }
        note.submitted = Some(at);
        let job = Job::Save {
            id,
            path: note.path.clone(),
            text: note.text.clone(),
            at,
        };
        if self.send(job) {
            self.writing += 1;
        } else {
            if let Some(note) = self.entries.iter_mut().find(|note| note.id == id) {
                note.submitted = None;
            }
            self.save_failed(id, at, i18n::t().note_storage_unavailable.into());
        }
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
                if note.submitted.is_some_and(|at| at >= last) {
                    return None;
                }
                if !all
                    && self
                        .save_errors
                        .get(&note.id)
                        .is_some_and(|failure| failure.at >= last)
                {
                    return None;
                }
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
                            submitted: None,
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
                Done::Saved { id, path, at } => {
                    self.writing = self.writing.saturating_sub(1);
                    if let Some(note) = self.entries.iter_mut().find(|note| note.id == id) {
                        note.path = Some(path);
                    }
                    self.acknowledge(id, at);
                }
                Done::Emptied { id, at } => {
                    self.writing = self.writing.saturating_sub(1);
                    if let Some(note) = self.entries.iter_mut().find(|note| note.id == id) {
                        note.path = None;
                    }
                    self.acknowledge(id, at);
                    if id != self.current
                        && self.entries.len() > 1
                        && self
                            .get(id)
                            .is_some_and(|note| note.is_blank() && note.dirty.is_none())
                    {
                        self.entries.retain(|note| note.id != id);
                    }
                }
                Done::Failed { id, at, error } => {
                    self.writing = self.writing.saturating_sub(1);
                    if let Some(note) = self.entries.iter_mut().find(|note| note.id == id) {
                        if note.submitted == Some(at) {
                            note.submitted = None;
                        }
                        self.save_failed(id, at, error);
                    }
                }
                // A deletion is no write, so the writes under way stay counted.
                Done::DeleteFailed(error) => self.delete_failed(error),
            }
        }
    }

    /// Keeps the failed write of edit `at`, unless a later edit of the note failed already.
    fn save_failed(&mut self, id: u64, at: Instant, error: String) {
        if self
            .save_errors
            .get(&id)
            .is_none_or(|failure| failure.at <= at)
        {
            self.failures += 1;
            let order = self.failures;
            self.save_errors.insert(id, Failure { at, order, error });
        }
        self.refresh_error();
    }

    fn delete_failed(&mut self, error: String) {
        self.failures += 1;
        self.delete_error = Some((self.failures, error));
        self.refresh_error();
    }

    fn acknowledge(&mut self, id: u64, at: Instant) {
        self.delete_error = None;
        if let Some(note) = self.entries.iter_mut().find(|note| note.id == id)
            && note.dirty.is_some_and(|(_, last)| last <= at)
        {
            note.dirty = None;
        }
        if self
            .save_errors
            .get(&id)
            .is_some_and(|failure| failure.at <= at)
        {
            self.save_errors.remove(&id);
        }
        self.refresh_error();
    }

    /// Shows the failure that arrived last among those still standing.
    fn refresh_error(&mut self) {
        let save = self
            .save_errors
            .values()
            .max_by_key(|failure| failure.order)
            .map(|failure| (failure.order, NoteError::Save(failure.error.clone())));
        let delete = self
            .delete_error
            .as_ref()
            .map(|(order, error)| (*order, NoteError::Delete(error.clone())));
        self.error = save
            .into_iter()
            .chain(delete)
            .max_by_key(|(order, _)| *order)
            .map(|(_, error)| error);
    }

    /// Writes every unwritten note and lets the worker finish, waiting at most `limit`.
    pub fn finish(&mut self, limit: Duration) {
        self.update();
        self.flush(true);
        self.jobs = None;
        let deadline = Instant::now() + limit;
        while let Some(worker) = &self.worker
            && !worker.is_finished()
            && Instant::now() < deadline
        {
            self.update();
            thread::sleep(Duration::from_millis(10));
        }
        self.update();
        if let Some(worker) = self.worker.take_if(|w| w.is_finished()) {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};

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
    fn a_failed_save_stays_pending_and_can_be_retried_after_the_disk_recovers() {
        let directory =
            std::env::temp_dir().join(format!("qqq-notes-retry-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::write(&directory, "temporary directory conflict").unwrap();
        let mut notes = Notes::new(directory.clone(), |_| None, || {});
        wait(&mut notes, |n| n.listed);
        type_text(&mut notes, "keep this edit");
        notes.flush(true);
        wait(&mut notes, |n| n.error.is_some() && n.writing == 0);
        assert!(!notes.saved());
        assert_eq!(notes.flush(false), None);
        assert_eq!(
            notes.writing, 0,
            "automatic repaint does not retry a failing write"
        );

        // A new edit after a failed attempt gets a fresh coalescing window too.
        notes.current_mut().dirty.as_mut().unwrap().0 =
            Instant::now() - SAVE_WAIT - Duration::from_secs(1);
        notes.edited();
        assert!(notes.flush(false).is_some());
        assert_eq!(notes.writing, 0);

        fs::remove_file(&directory).unwrap();
        fs::create_dir(&directory).unwrap();
        notes.flush(true);
        wait(&mut notes, |n| n.saved());
        assert_eq!(notes.error, None);
        assert_eq!(
            fs::read_to_string(notes.current().path.as_ref().unwrap()).unwrap(),
            "keep this edit"
        );
        notes.finish(Duration::from_secs(5));
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn an_acknowledgement_keeps_later_edits_and_another_notes_failure() {
        let directory = std::env::temp_dir().join(format!("qqq-notes-acks-{}", std::process::id()));
        let mut notes = Notes::new(directory.clone(), |_| None, || {});
        wait(&mut notes, |n| n.listed);
        let first = notes.current;
        type_text(&mut notes, "first edit");
        let first_at = notes.current().dirty.unwrap().1;
        type_text(&mut notes, "later edit");
        notes.acknowledge(first, first_at);
        assert!(!notes.saved());

        let other = notes.add();
        notes.save_failed(other, first_at, "locked".into());
        notes.delete_failed("denied".into());
        let latest = notes.get(first).unwrap().dirty.unwrap().1;
        notes.acknowledge(first, latest);
        assert_eq!(notes.delete_error, None);
        assert_eq!(notes.error, Some(NoteError::Save("locked".into())));
        notes.delete(other);
        notes.finish(Duration::from_secs(5));
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn the_latest_standing_failure_is_shown() {
        let directory =
            std::env::temp_dir().join(format!("qqq-notes-latest-{}", std::process::id()));
        let mut notes = Notes::new(directory.clone(), |_| None, || {});
        wait(&mut notes, |n| n.listed);
        let older = notes.add();
        let newer = notes.add();
        let at = Instant::now();

        // A failure of a note with a smaller id still shows when it arrives last.
        notes.save_failed(newer, at, "locked".into());
        notes.save_failed(older, at, "full".into());
        assert_eq!(notes.error, Some(NoteError::Save("full".into())));
        notes.delete_failed("denied".into());
        assert_eq!(notes.error, Some(NoteError::Delete("denied".into())));

        // A successful write clears the deletion error and that note's failure; the
        // latest failure left takes its place.
        notes.acknowledge(older, at);
        assert_eq!(notes.error, Some(NoteError::Save("locked".into())));
        notes.acknowledge(newer, at);
        assert_eq!(notes.error, None);
        notes.finish(Duration::from_secs(5));
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn edits_after_a_slow_submission_start_a_new_debounce_window() {
        let directory =
            std::env::temp_dir().join(format!("qqq-notes-slow-write-{}", std::process::id()));
        let mut notes = Notes::new(directory.clone(), |_| None, || {});
        let (jobs, inbox) = mpsc::channel();
        let (answers, done) = mpsc::channel();
        notes.jobs = Some(jobs);
        notes.done = done;

        // Keep the storage answers pending while the first batch reaches SAVE_WAIT.
        let old = Instant::now() - SAVE_WAIT - Duration::from_secs(1);
        notes.current_mut().text = "first batch".into();
        notes.current_mut().dirty = Some((old, old));
        notes.flush(false);
        assert!(matches!(inbox.try_recv(), Ok(Job::Save { at, .. }) if at == old));
        assert_eq!(notes.writing, 1);

        for text in ["n", "ne", "new", "newest edit"] {
            type_text(&mut notes, text);
            assert!(notes.current().dirty.unwrap().0 > old);
            assert!(notes.flush(false).is_some());
            assert!(inbox.try_recv().is_err());
            assert_eq!(notes.writing, 1);
        }
        notes.flush(true);
        let Ok(Job::Save { id, text, at, .. }) = inbox.try_recv() else {
            panic!("the latest edit is submitted once")
        };
        assert_eq!(text, "newest edit");
        assert!(inbox.try_recv().is_err());
        answers
            .send(Done::Saved {
                id,
                path: directory.join("note.md"),
                at: old,
            })
            .unwrap();
        notes.update();
        assert!(
            !notes.saved(),
            "the older acknowledgement keeps the newest edit pending"
        );
        answers
            .send(Done::Saved {
                id,
                path: directory.join("note.md"),
                at,
            })
            .unwrap();
        notes.update();
        assert!(notes.saved());
        notes.finish(Duration::from_secs(5));
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
        let id = notes.current;
        let at = Instant::now();
        notes.entries[0].dirty = Some((at, at));
        answers
            .send(Done::Failed {
                id,
                at,
                error: "full".into(),
            })
            .unwrap();
        notes.update();
        assert_eq!(notes.writing, 0);
        assert_eq!(notes.error, Some(NoteError::Save("full".into())));
        assert!(!notes.saved());
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
