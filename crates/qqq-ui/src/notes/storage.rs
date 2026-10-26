//! Markdown file operations owned by the storage worker.

use super::*;
use std::{collections::HashMap, fs, io, path::Path, time::UNIX_EPOCH};

pub(super) struct Worker {
    pub(super) directory: PathBuf,
    pub(super) outbox: mpsc::Sender<Done>,
    pub(super) local_time: LocalTime,
    pub(super) wake: Box<dyn Fn() + Send>,
    /// Files this worker created, by note; a write that set out before the GUI learnt the
    /// name still finds the file.
    pub(super) paths: HashMap<u64, PathBuf>,
}

impl Worker {
    pub(super) fn run(&mut self, job: Job) {
        match job {
            Job::List => {
                let files = self.files();
                self.send(Done::Listed(files));
            }
            Job::Save { id, path, text, at } => {
                let path = self.paths.get(&id).cloned().or(path);
                let done = if text.trim().is_empty() {
                    match path.as_ref().map(fs::remove_file) {
                        Some(Err(error)) if error.kind() != io::ErrorKind::NotFound => {
                            Done::Failed {
                                id,
                                at,
                                error: error.to_string(),
                            }
                        }
                        _ => {
                            self.paths.remove(&id);
                            Done::Emptied { id, at }
                        }
                    }
                } else {
                    match self.save(path, &text) {
                        Ok(path) => {
                            self.paths.insert(id, path.clone());
                            Done::Saved { id, path, at }
                        }
                        Err(error) => Done::Failed {
                            id,
                            at,
                            error: error.to_string(),
                        },
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
