//! The preferences file, written behind whoever changed it.
//!
//! Every setting lives in one TOML file, so every change is a read, an edit and a write. A change
//! is handed to [`PreferencesFile::edit`], which returns at once, and one writer thread saves it:
//! nothing a person presses waits on the disk, and two changes made together never overwrite each
//! other. The writer applies every edit still waiting to what the file holds as it writes, so a
//! field another process wrote meanwhile (a background pass's notification marks) survives.
//!
//! A read ([`PreferencesFile::load`]) is the file with every waiting edit applied, so what was just
//! changed is what is read. An edit must therefore set a value rather than adjust one: it can be
//! applied to a read and to the save that follows it.
//!
//! A save that fails keeps its edits waiting. They still apply to every read, and are written with
//! the next change or a [`retry`](PreferencesFile::retry); while one waits, [`failed`] is true and
//! the app raises a notice.
//!
//! [`failed`]: PreferencesFile::failed

use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, MutexGuard},
};

use engine_api::Provider;
use mailcal_account::{Preferences, load_preferences, save_preferences};

use crate::App;

/// One change to the preferences: it sets values, so applying it twice is applying it once.
type Edit = Arc<dyn Fn(&mut Preferences) + Send + Sync>;

/// A handle on the preferences file; every clone names the same file and the same writer. One
/// with no file (the demo, the showcase and most tests) keeps nothing.
#[derive(Clone, Default)]
pub(crate) struct PreferencesFile(Option<Arc<Handle>>);

/// The last handle going stops the writer, once what is waiting has been written.
struct Handle(Arc<Shared>, bool);

struct Shared {
    path: PathBuf,
    /// Held while the file is read or written, so a read never sees a save half done, nor the
    /// file and the waiting edits from two different moments. Never held by
    /// [`PreferencesFile::edit`].
    io: Mutex<()>,
    queue: Mutex<Queue>,
    /// Wakes the writer when something is due, and anyone waiting for it to finish.
    moved: Condvar,
    /// Told whenever whether a save has failed changes.
    on_failure: Box<dyn Fn() + Send + Sync>,
}

#[derive(Default)]
struct Queue {
    /// The edits not yet in the file, oldest first.
    pending: Vec<Edit>,
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

impl Queue {
    fn due(&self) -> bool {
        !self.pending.is_empty() && self.asked != self.attempted
    }
}

impl PreferencesFile {
    /// The file at `path`, with its writer started. `on_failure` is told whenever a save fails
    /// after none had, and when one succeeds after a failure.
    pub(crate) fn open(path: PathBuf, on_failure: impl Fn() + Send + Sync + 'static) -> Self {
        Self::start(path, Box::new(on_failure), false)
    }

    /// The file at `path` for a test that reads the disk straight after a change: every edit
    /// waits for its write.
    #[cfg(test)]
    pub(crate) fn writing_through(path: Option<PathBuf>) -> Self {
        path.map_or_else(Self::default, |path| {
            Self::start(path, Box::new(|| {}), true)
        })
    }

    fn start(path: PathBuf, on_failure: Box<dyn Fn() + Send + Sync>, settle_each: bool) -> Self {
        let shared = Arc::new(Shared {
            path,
            io: Mutex::new(()),
            queue: Mutex::new(Queue::default()),
            moved: Condvar::new(),
            on_failure,
        });
        let writer = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("preferences".to_owned())
            .spawn(move || write_until_stopped(&writer))
            .expect("the preferences writer could not be started");
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

    /// The preferences as they stand: the file, with every edit not yet in it applied. Defaults
    /// for no file, or one that is missing or does not parse.
    pub(crate) fn load(&self) -> Preferences {
        let Some(handle) = &self.0 else {
            return Preferences::default();
        };
        let shared = &handle.0;
        let _io = lock(&shared.io);
        let pending = lock(&shared.queue).pending.clone();
        let mut prefs = load_preferences(&shared.path);
        for edit in &pending {
            edit(&mut prefs);
        }
        prefs
    }

    /// Applies `edit` to every read from now on, and has it saved behind the caller.
    pub(crate) fn edit(&self, edit: impl Fn(&mut Preferences) + Send + Sync + 'static) {
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
            queue = shared
                .moved
                .wait(queue)
                .expect("preferences queue poisoned");
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        lock(&self.0.queue).stopping = true;
        self.0.moved.notify_all();
    }
}

/// The writer: saves whatever is due, one attempt at a time, until the last handle has gone and
/// nothing is due.
fn write_until_stopped(shared: &Shared) {
    loop {
        {
            let mut queue = lock(&shared.queue);
            while !queue.due() && !queue.stopping {
                queue = shared
                    .moved
                    .wait(queue)
                    .expect("preferences queue poisoned");
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
        let mut prefs = load_preferences(&shared.path);
        for edit in &batch {
            edit(&mut prefs);
        }
        let saved = save_preferences(&shared.path, &prefs);
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
                "preferences: a change could not be saved ({:?})",
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

impl<P: Provider> App<P> {
    /// Whether a change to the settings could not be saved, so it holds only until the app
    /// closes. The notices say so; [`Surface::Notices`](crate::Surface::Notices) is signalled when
    /// this changes.
    #[must_use]
    pub fn preferences_not_saved(&self) -> bool {
        self.prefs.failed()
    }

    /// Tries the failed save again, and waits until the attempt has ended. Not for a UI thread.
    pub fn retry_preferences_save(&self) {
        self.prefs.retry();
    }

    /// Waits until every change to the settings has been written, or a write has failed. For a
    /// process about to end, and for tests. Not for a UI thread.
    pub fn settle_preferences(&self) {
        self.prefs.settle();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().expect("preferences mutex poisoned")
}

#[cfg(test)]
#[path = "preferences_file_tests.rs"]
mod tests;
