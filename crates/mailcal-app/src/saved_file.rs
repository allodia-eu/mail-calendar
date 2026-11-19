//! A settings file, written behind whoever changed it.
//!
//! A change is handed to [`SavedFile::edit`], which returns at once, and one writer thread saves
//! it: nothing a person presses waits on the disk, and two changes made together never overwrite
//! each other. The writer applies every edit still waiting to what the file holds as it writes, so
//! a field another process wrote meanwhile (a background pass's notification marks) survives.
//!
//! A read ([`SavedFile::load`]) is the file with every waiting edit applied, so what was just
//! changed is what is read. An edit must therefore set a value rather than adjust one: it can be
//! applied to a read and to the save that follows it.
//!
//! A save that fails keeps its edits waiting. They still apply to every read, and are written with
//! the next change or a [`retry`](SavedFile::retry); while one waits, [`failed`] is true and the
//! app raises a notice.
//!
//! [`failed`]: SavedFile::failed

use std::{
    io,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, MutexGuard},
    time::Instant,
};

/// What a [`SavedFile`] holds: read whole and written whole.
pub(crate) trait Document: Default + Send + 'static {
    /// What the writer thread and its log lines are called.
    const NAME: &'static str;

    /// The file at `path`, or the defaults when it is missing or does not parse.
    fn read(path: &Path) -> Self;

    /// Writes the whole document to `path`.
    fn write(path: &Path, document: &Self) -> io::Result<()>;
}

/// One change to a document: it sets values, so applying it twice is applying it once.
type Edit<D> = Arc<dyn Fn(&mut D) + Send + Sync>;

/// A handle on one file; every clone names the same file and the same writer. One with no file
/// (the demo, the showcase and most tests) keeps nothing.
pub(crate) struct SavedFile<D: Document>(Option<Arc<Handle<D>>>);

impl<D: Document> Clone for SavedFile<D> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<D: Document> Default for SavedFile<D> {
    fn default() -> Self {
        Self(None)
    }
}

/// The last handle going stops the writer, once what is waiting has been written.
struct Handle<D: Document>(Arc<Shared<D>>, bool);

struct Shared<D: Document> {
    path: PathBuf,
    /// Held while the file is read or written, so a read never sees a save half done, nor the
    /// file and the waiting edits from two different moments. Never held by [`SavedFile::edit`].
    io: Mutex<()>,
    queue: Mutex<Queue<D>>,
    /// Wakes the writer when something is due, and anyone waiting for it to finish.
    moved: Condvar,
    /// Told whenever whether a save has failed changes.
    on_failure: Box<dyn Fn() + Send + Sync>,
}

struct Queue<D> {
    /// The edits not yet in the file, oldest first.
    pending: Vec<Edit<D>>,
    /// Bumped by every edit and every retry: the writer tries again only when it has moved past
    /// the attempt that failed.
    asked: u64,
    /// The `asked` the last attempt covered.
    attempted: u64,
    /// The last save failed and its edits are waiting.
    failed: bool,
    /// An attempt is under way.
    writing: bool,
    stopping: bool,
}

impl<D> Default for Queue<D> {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            asked: 0,
            attempted: 0,
            failed: false,
            writing: false,
            stopping: false,
        }
    }
}

impl<D> Queue<D> {
    fn due(&self) -> bool {
        !self.pending.is_empty() && self.asked != self.attempted
    }
}

impl<D: Document> SavedFile<D> {
    /// The file at `path`, with its writer started. `on_failure` is told whenever a save fails
    /// after none had, and when one succeeds after a failure.
    pub(crate) fn open(path: PathBuf, on_failure: impl Fn() + Send + Sync + 'static) -> Self {
        Self::start(path, Box::new(on_failure), false)
    }

    /// The file at `path`; with `settle_each`, every change is written before
    /// [`edit`](Self::edit) returns, so a test can read the disk straight after one.
    pub(crate) fn start(
        path: PathBuf,
        on_failure: Box<dyn Fn() + Send + Sync>,
        settle_each: bool,
    ) -> Self {
        let shared = Arc::new(Shared {
            path,
            io: Mutex::new(()),
            queue: Mutex::new(Queue::default()),
            moved: Condvar::new(),
            on_failure,
        });
        let writer = Arc::clone(&shared);
        std::thread::Builder::new()
            .name(D::NAME.to_owned())
            .spawn(move || write_until_stopped(&writer))
            .expect("a settings writer could not be started");
        Self(Some(Arc::new(Handle(shared, settle_each))))
    }

    /// The file named by `path`, or one that keeps nothing for `None`.
    pub(crate) fn at(path: Option<PathBuf>, on_failure: impl Fn() + Send + Sync + 'static) -> Self {
        path.map_or_else(Self::default, |path| Self::open(path, on_failure))
    }

    /// Whether this handle names a file at all.
    pub(crate) fn is_stored(&self) -> bool {
        self.0.is_some()
    }

    /// Whether there is anything to read: the file, or a change not yet in it.
    pub(crate) fn exists(&self) -> bool {
        self.0.as_ref().is_some_and(|handle| {
            let shared = &handle.0;
            let _io = lock(&shared.io);
            !lock(&shared.queue).pending.is_empty() || shared.path.exists()
        })
    }

    /// The document as it stands: the file, with every edit not yet in it applied. Defaults for
    /// no file, or one that is missing or does not parse.
    pub(crate) fn load(&self) -> D {
        let Some(handle) = &self.0 else {
            return D::default();
        };
        let shared = &handle.0;
        let _io = lock(&shared.io);
        let pending = lock(&shared.queue).pending.clone();
        let mut document = D::read(&shared.path);
        for edit in &pending {
            edit(&mut document);
        }
        document
    }

    /// Applies `edit` to every read from now on, and has it saved behind the caller.
    pub(crate) fn edit(&self, edit: impl Fn(&mut D) + Send + Sync + 'static) {
        let Some(handle) = &self.0 else {
            return;
        };
        let shared = &handle.0;
        let mut queue = lock(&shared.queue);
        queue.pending.push(Arc::new(edit));
        queue.asked += 1;
        shared.moved.notify_all();
        drop(queue);
        if handle.1 {
            self.settle();
        }
    }

    /// Whether the last save failed, so a change is held only in memory.
    pub(crate) fn failed(&self) -> bool {
        self.0
            .as_ref()
            .is_some_and(|handle| lock(&handle.0.queue).failed)
    }

    /// Tries a failed save again, and waits until that attempt has ended. Not for a UI thread.
    pub(crate) fn retry(&self) {
        let Some(handle) = &self.0 else {
            return;
        };
        let shared = &handle.0;
        lock(&shared.queue).asked += 1;
        shared.moved.notify_all();
        self.settle();
    }

    /// Waits until nothing is due: every edit saved, or the last attempt failed. For a pass whose
    /// process may end once it returns, and for tests. Not for a UI thread.
    pub(crate) fn settle(&self) {
        let Some(handle) = &self.0 else {
            return;
        };
        let shared = &handle.0;
        let mut queue = lock(&shared.queue);
        while queue.due() || queue.writing {
            queue = shared.moved.wait(queue).expect("settings queue poisoned");
        }
    }

    /// [`settle`](Self::settle), until `deadline` at the latest. Whether nothing was left to
    /// write when it returned.
    pub(crate) fn settle_by(&self, deadline: Instant) -> bool {
        let Some(handle) = &self.0 else {
            return true;
        };
        let shared = &handle.0;
        let mut queue = lock(&shared.queue);
        while queue.due() || queue.writing {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            queue = shared
                .moved
                .wait_timeout(queue, left)
                .expect("settings queue poisoned")
                .0;
        }
        true
    }
}

#[cfg(test)]
impl<D: Document> SavedFile<D> {
    /// [`settle`](Self::settle), for no longer than `limit`. Whether nothing was left to write
    /// when it returned.
    pub(crate) fn settle_within(&self, limit: std::time::Duration) -> bool {
        self.settle_by(Instant::now() + limit)
    }

    /// Holds the file as a save does, so the writer cannot take it: a disk that does not answer.
    pub(crate) fn hold_disk(&self) -> MutexGuard<'_, ()> {
        lock(&self.0.as_ref().expect("a stored file").0.io)
    }
}

impl<D: Document> Drop for Handle<D> {
    fn drop(&mut self) {
        lock(&self.0.queue).stopping = true;
        self.0.moved.notify_all();
    }
}

/// The writer: saves whatever is due, one attempt at a time, until the last handle has gone and
/// nothing is due.
fn write_until_stopped<D: Document>(shared: &Shared<D>) {
    loop {
        {
            let mut queue = lock(&shared.queue);
            while !queue.due() && !queue.stopping {
                queue = shared.moved.wait(queue).expect("settings queue poisoned");
            }
            if !queue.due() {
                return;
            }
        }
        // Taken in the order every reader takes them: the file first.
        let io = lock(&shared.io);
        let batch = {
            let mut queue = lock(&shared.queue);
            queue.attempted = queue.asked;
            queue.writing = true;
            queue.pending.clone()
        };
        let mut document = D::read(&shared.path);
        for edit in &batch {
            edit(&mut document);
        }
        let saved = D::write(&shared.path, &document);
        let flipped = {
            let mut queue = lock(&shared.queue);
            if saved.is_ok() {
                queue.pending.drain(..batch.len());
            }
            let flipped = queue.failed == saved.is_ok();
            queue.failed = saved.is_err();
            flipped
        };
        drop(io);
        if let Err(err) = saved {
            log::warn!(
                "{}: a change could not be saved ({:?})",
                D::NAME,
                err.kind()
            );
        }
        // Told before the attempt counts as ended, so whoever waited on it reads what it said.
        if flipped {
            (shared.on_failure)();
        }
        lock(&shared.queue).writing = false;
        shared.moved.notify_all();
    }
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().expect("settings mutex poisoned")
}
