//! Crash recovery: a journal of the open project, kept up to date after
//! every action, so a crash loses at most the action being written.
//!
//! Each editing session owns a folder under a recovery root (the app puts
//! it in its data folder):
//!
//! ```text
//! recovery/
//! └── 1234-1739372400123/      one per session
//!     ├── lock                 held while the session is running
//!     ├── session.json         what the project is and where it is saved
//!     ├── project.fnm/         the project as of the last action
//!     └── history/             its undo history (see `crate::history`)
//! ```
//!
//! Writes happen on a background thread, so the interface never waits for
//! the disk. Every file is replaced atomically, and only top-level layers
//! that changed are rewritten, so recording an action costs about one small
//! file. A session that ends normally deletes its folder. At the next start,
//! a folder whose lock is free belonged to a session that crashed, and if
//! it had unsaved work the app offers to recover it.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use fanimate_core::{Document, LayerId};
use serde::{Deserialize, Serialize};

use crate::history::{self, HistoryRecord, HistoryWriter};
use crate::io::{document_file, io_err, layer_file, manifest, write_json_atomic};
use crate::{Report, Result};

const LOCK: &str = "lock";
const SESSION: &str = "session.json";
const PROJECT: &str = "project.fnm";
const HISTORY: &str = "history";

/// What a recovery folder holds, written beside the project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// The project's name.
    pub name: String,
    /// Where the project is saved; `None` if it never was.
    pub path: Option<PathBuf>,
    /// Whether there were changes not yet saved to `path`.
    pub unsaved: bool,
    /// When the journal was last written, in seconds since 1970.
    pub updated: u64,
}

struct Snapshot {
    doc: Box<Document>,
    info: SessionInfo,
    history: Option<HistoryRecord>,
}

enum Message {
    Record(Snapshot),
    /// Reply once everything before this has been written.
    Flush(Sender<()>),
}

/// The journal of one editing session.
pub struct Journal {
    dir: PathBuf,
    tx: Option<Sender<Message>>,
    worker: Option<JoinHandle<()>>,
    /// The last write error, for the app to show.
    error: Arc<Mutex<Option<String>>>,
    /// Held for the life of the session, so other instances know it is
    /// still running. Released by the OS if the process dies.
    _lock: File,
}

impl Journal {
    /// Start a session in a new folder under `root`.
    pub fn start(root: &Path) -> Result<Self> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let dir = root.join(format!("{}-{stamp}", std::process::id()));
        fs::create_dir_all(dir.join(PROJECT).join("layers")).map_err(|e| io_err(&dir, e))?;
        let lock_path = dir.join(LOCK);
        let lock = File::create(&lock_path).map_err(|e| io_err(&lock_path, e))?;
        lock.lock().map_err(|e| io_err(&lock_path, e))?;

        let (tx, rx) = mpsc::channel();
        let error = Arc::new(Mutex::new(None));
        let worker = {
            let (dir, error) = (dir.clone(), Arc::clone(&error));
            std::thread::Builder::new()
                .name("fanimate-journal".into())
                .spawn(move || Writer::new(dir).run(rx, &error))
                .map_err(|e| io_err(root, e))?
        };
        Ok(Self {
            dir,
            tx: Some(tx),
            worker: Some(worker),
            error,
            _lock: lock,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Record the project as it is now, with its undo history if given.
    /// Returns at once; the writing happens in the background.
    pub fn record(&self, doc: &Document, path: Option<&Path>, unsaved: bool, history: Option<HistoryRecord>) {
        let info = SessionInfo {
            name: doc.name.clone(),
            path: path.map(Path::to_path_buf),
            unsaved,
            updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        if let Some(tx) = &self.tx {
            let _ = tx.send(Message::Record(Snapshot {
                doc: Box::new(doc.mainline().into_owned()),
                info,
                history: history.filter(HistoryRecord::is_consistent),
            }));
        }
    }

    /// Wait until everything recorded so far is on disk.
    pub fn flush(&self) {
        let (done, wait) = mpsc::channel();
        if let Some(tx) = &self.tx
            && tx.send(Message::Flush(done)).is_ok()
        {
            let _ = wait.recv();
        }
    }

    /// The last error writing the journal, if the latest write failed.
    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|e| e.clone())
    }

    /// End the session normally: its work is saved or deliberately
    /// discarded, so there is nothing to recover. Deletes the folder.
    pub fn close(mut self) {
        self.stop();
        let _ = fs::remove_dir_all(&self.dir);
    }

    fn stop(&mut self) {
        drop(self.tx.take());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for Journal {
    /// Without [`Journal::close`] (a panic, say) the folder stays, so the
    /// work can be recovered.
    fn drop(&mut self) {
        self.stop();
    }
}

/// The background writer. Remembers what it last wrote, so it rewrites only
/// the top-level layers that changed.
struct Writer {
    project: PathBuf,
    session: PathBuf,
    history: HistoryWriter,
    last: Option<Document>,
}

impl Writer {
    fn new(dir: PathBuf) -> Self {
        Self {
            project: dir.join(PROJECT),
            session: dir.join(SESSION),
            history: HistoryWriter::new(dir.join(HISTORY)),
            last: None,
        }
    }

    fn run(mut self, rx: Receiver<Message>, error: &Mutex<Option<String>>) {
        // only the newest state matters: when several arrive before the
        // disk catches up, the older ones are skipped
        let mut pending: Option<Snapshot> = None;
        loop {
            let msg = if pending.is_some() {
                rx.try_recv().ok()
            } else {
                match rx.recv() {
                    Ok(msg) => Some(msg),
                    Err(_) => break, // the session ended
                }
            };
            match msg {
                Some(Message::Record(snapshot)) => pending = Some(snapshot),
                Some(Message::Flush(done)) => {
                    self.write_pending(&mut pending, error);
                    let _ = done.send(());
                }
                None => self.write_pending(&mut pending, error),
            }
        }
    }

    fn write_pending(&mut self, pending: &mut Option<Snapshot>, error: &Mutex<Option<String>>) {
        if let Some(snapshot) = pending.take() {
            let result = self
                .write(&snapshot.doc, &snapshot.info)
                .and_then(|()| match &snapshot.history {
                    Some(h) => self.history.write(h),
                    None => Ok(()),
                });
            if let Ok(mut e) = error.lock() {
                *e = result.err().map(|e| e.to_string());
            }
        }
    }

    fn write(&mut self, doc: &Document, info: &SessionInfo) -> Result<()> {
        if self.last.is_none() {
            write_json_atomic(&self.project.join("manifest.json"), &manifest())?;
        }
        let previous = self.last.as_ref();
        let unchanged = |id: LayerId| {
            let old = previous.and_then(|p| p.layers.iter().find(|l| l.id == id));
            let new = doc.layers.iter().find(|l| l.id == id);
            old.is_some() && old == new
        };
        // layers first, then the document that lists them, so the document
        // never names a layer file that is not there
        for layer in doc.layers.iter().filter(|l| !unchanged(l.id)) {
            write_json_atomic(&self.project.join(layer_file(layer)), layer)?;
        }
        write_json_atomic(&self.project.join("document.json"), &document_file(doc))?;
        if let Some(previous) = previous {
            for gone in previous
                .layers
                .iter()
                .filter(|old| !doc.layers.iter().any(|l| l.id == old.id))
            {
                let _ = fs::remove_file(self.project.join(layer_file(gone)));
            }
        }
        write_json_atomic(&self.session, info)?;
        self.last = Some(doc.clone());
        Ok(())
    }
}

/// Work left behind by a session that crashed.
#[derive(Clone, Debug, PartialEq)]
pub struct Recoverable {
    pub dir: PathBuf,
    pub info: SessionInfo,
}

impl Recoverable {
    /// Load the recovered project, repairing what the validator can.
    pub fn load(&self) -> Result<(Document, Report)> {
        crate::load_and_repair(self.dir.join(PROJECT))
    }

    /// The recovered project's undo history, if it was kept and every state
    /// in it is sound. A history that is missing, damaged or would need
    /// repairs is left out rather than risk undoing into a broken state;
    /// the project itself is unaffected.
    pub fn load_history(&self) -> Option<HistoryRecord> {
        let record = history::read(&self.dir.join(HISTORY)).ok()?;
        let sound = record.is_consistent()
            && record.states.iter().all(|s| {
                crate::validate(&s.document())
                    .unresolved_errors()
                    .next()
                    .is_none()
            });
        sound.then_some(record)
    }

    /// Delete the recovery folder, once its work has been recovered or the
    /// user has chosen to discard it.
    pub fn discard(&self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Sessions under `root` that ended without closing (their lock is free)
/// and had unsaved work, newest first. Ended sessions with nothing unsaved
/// are cleaned up. Folders of running sessions are left alone.
pub fn find_recoverable(root: &Path) -> Vec<Recoverable> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for dir in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        // a running session holds its lock; a crashed one does not
        let Ok(lock) = File::options().read(true).write(true).open(dir.join(LOCK)) else {
            continue;
        };
        if lock.try_lock().is_err() {
            continue;
        }
        let info = fs::read_to_string(dir.join(SESSION))
            .ok()
            .and_then(|text| serde_json::from_str::<SessionInfo>(&text).ok());
        let has_project = dir.join(PROJECT).join("document.json").is_file();
        drop(lock);
        match info {
            Some(info) if info.unsaved && has_project => found.push(Recoverable { dir, info }),
            // nothing unsaved, or nothing usable: nothing to offer
            _ => {
                let _ = fs::remove_dir_all(&dir);
            }
        }
    }
    found.sort_by_key(|r| std::cmp::Reverse(r.info.updated));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use fanimate_core::{LayerKind, samples};
    use std::process::Command;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fanimate-journal-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// A document after `n` edits: each edit adds a layer, and renames one.
    fn edited(n: usize) -> Document {
        let mut doc = samples::hello();
        for i in 0..n {
            if i % 2 == 0 {
                let layer = doc.new_layer(format!("Layer {i}"), LayerKind::vector(vec![]));
                doc.layers.push(layer);
            } else {
                doc.layers[0].name = format!("renamed {i}");
            }
        }
        doc
    }

    #[test]
    fn journal_holds_the_last_recorded_state() {
        let root = temp_root("last");
        let journal = Journal::start(&root).unwrap();
        for n in 0..10 {
            journal.record(&edited(n), Some(Path::new("scene.fnm")), true, None);
        }
        journal.flush();
        assert!(journal.error().is_none());
        // a running session is never offered for recovery
        assert!(find_recoverable(&root).is_empty());
        let (doc, report) = crate::load(journal.dir().join(PROJECT)).unwrap();
        assert_eq!(doc, edited(9));
        assert!(report.is_clean(), "{report}");
        journal.close();
        assert!(fs::read_dir(&root).unwrap().next().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn removed_layers_are_removed_from_the_journal() {
        let root = temp_root("removed");
        let journal = Journal::start(&root).unwrap();
        journal.record(&edited(5), None, true, None);
        journal.flush();
        journal.record(&edited(1), None, true, None);
        journal.flush();
        let layers = fs::read_dir(journal.dir().join(PROJECT).join("layers"))
            .unwrap()
            .count();
        assert_eq!(layers, edited(1).layers.len());
        journal.close();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn saved_sessions_left_behind_are_cleaned_up_not_offered() {
        let root = temp_root("saved");
        let journal = Journal::start(&root).unwrap();
        journal.record(&edited(2), Some(Path::new("scene.fnm")), false, None);
        journal.flush();
        drop(journal); // ends without close(), as in a crash
        assert!(find_recoverable(&root).is_empty());
        assert!(fs::read_dir(&root).unwrap().next().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    const CHILD: &str = "FANIMATE_JOURNAL_CRASH_CHILD";

    /// Helper for the crash test below: in a child process, records `n`
    /// edits, waits for the last to be written, and aborts like a crash.
    /// Does nothing when run as a normal test.
    #[test]
    fn crash_child() {
        let Ok(spec) = std::env::var(CHILD) else { return };
        let (root, n) = spec.split_once('|').unwrap();
        let journal = Journal::start(Path::new(root)).unwrap();
        let n: usize = n.parse().unwrap();
        let mut record = HistoryRecord::default();
        for i in 0..=n {
            // an undo history like the editor's: one state per edit
            let like = record.states.last().cloned();
            record
                .states
                .push(crate::HistoryState::capture(&edited(i), &[], like.as_ref()));
            if i > 0 {
                record.labels.push(format!("Edit {i}"));
            }
            record.cursor = i;
            journal.record(
                &edited(i),
                Some(Path::new("scene.fnm")),
                true,
                Some(record.clone()),
            );
            if i == n {
                journal.flush();
            }
        }
        std::process::abort();
    }

    /// The M2 exit check, in miniature: a process killed mid-session leaves
    /// a journal that opens, cleanly, with the last action in it.
    #[test]
    fn work_survives_a_crashed_process() {
        let root = temp_root("crash");
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "journal::tests::crash_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, format!("{}|17", root.display()))
            .status()
            .unwrap();
        assert!(!status.success(), "the child should have aborted");

        let found = find_recoverable(&root);
        assert_eq!(found.len(), 1);
        let r = &found[0];
        assert_eq!(r.info.path.as_deref(), Some(Path::new("scene.fnm")));
        assert!(r.info.unsaved);
        let (doc, report) = r.load().unwrap();
        assert!(report.is_clean(), "{report}");
        assert_eq!(doc, edited(17));
        // and so does its undo history, back to the start
        let history = r.load_history().unwrap();
        assert_eq!(history.cursor, 17);
        assert_eq!(history.labels.len(), 17);
        assert_eq!(history.states[0].document(), edited(0));
        assert_eq!(history.states[17].document(), doc);
        r.discard();
        assert!(find_recoverable(&root).is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
