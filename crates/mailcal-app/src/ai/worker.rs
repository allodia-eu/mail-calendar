//! The work that sends to a model: learning a style and drafting a reply (`docs/ai.md` rules 13
//! and 20), off the host's thread.
//!
//! Each key is a lane of its own: one learning run at a time, one draft per composition, and a
//! draft never waits behind a learning run or another composition's draft. A piece of work is
//! queued on the host's thread and returns at once; the caller told to runs it on a thread of its
//! own. Work asked for again while the same work runs or waits is dropped, so a double press
//! starts it once. Stopping work that waits takes it out of the queue; stopping work that runs
//! sets its [`AiCancel`], which abandons the request in flight and which the work reads before its
//! next. Work asked for again while the stopped run winds down waits for it, then runs. What a
//! piece of work came to is its own surface's to record: the queue only says what runs and what
//! waits, and signals the surface it belongs to as that moves.

use std::{collections::VecDeque, fmt, future::Future};

use engine_api::Provider;
use mailcal_ai::HttpTransport;

use super::{AiCancel, AiHost};
use crate::{App, CompositionId, Surface};

/// Which work, and so which surface draws it. Two pieces of work with one key are the same work.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AiWorkKey {
    /// Learning a style; one run at a time whatever its account or range.
    Learning,
    /// Drafting the reply a composition is writing.
    Drafting(CompositionId),
}

impl AiWorkKey {
    /// The surface that draws this work.
    #[must_use]
    pub const fn surface(&self) -> Surface {
        match self {
            Self::Learning => Surface::WritingStyle,
            Self::Drafting(_) => Surface::ReplyDraft,
        }
    }
}

/// The body of a piece of work: what it does with the app and its run.
type Body<P> = Box<dyn FnOnce(&App<P>, &AiRun<'_>) + Send>;

/// A piece of work for the queue.
pub struct AiWork<P> {
    key: AiWorkKey,
    body: Body<P>,
}

impl<P> AiWork<P> {
    /// Work under `key` that runs `body`. The body asks [`App::ai_backend`] for where to send,
    /// reads [`AiRun::cancelled`] between requests, and records what it came to on its surface.
    pub fn new(key: AiWorkKey, body: impl FnOnce(&App<P>, &AiRun<'_>) + Send + 'static) -> Self {
        Self {
            key,
            body: Box::new(body),
        }
    }

    /// Its key.
    #[must_use]
    pub fn key(&self) -> &AiWorkKey {
        &self.key
    }
}

impl<P> fmt::Debug for AiWork<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiWork")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

/// What a piece of work runs with: its stop and the host's runtime and socket.
pub struct AiRun<'a> {
    cancel: AiCancel,
    host: &'a dyn AiHost,
}

impl fmt::Debug for AiRun<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiRun")
            .field("cancel", &self.cancel)
            .finish_non_exhaustive()
    }
}

impl AiRun<'_> {
    /// Whether the person stopped this work. Read before each request: work that was stopped
    /// keeps nothing.
    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.cancel.is_set()
    }

    /// The stop itself, for `mailcal_ai::LearnOptions::cancel`.
    #[must_use]
    pub fn cancel(&self) -> &AiCancel {
        &self.cancel
    }

    /// Runs `work` on the app's runtime and returns what it came to: for the engine's reads
    /// between requests.
    pub fn wait<T: Send>(&self, work: impl Future<Output = T> + Send) -> T {
        let mut answer = None;
        self.host.wait(Box::pin(async {
            answer = Some(work.await);
        }));
        answer.expect("the host runs work to its end")
    }

    /// A transport whose request in flight is abandoned when this work is stopped.
    pub(crate) fn transport(&self) -> Box<dyn HttpTransport> {
        self.host.transport(self.cancel.clone())
    }
}

/// What queueing a piece of work did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiQueued {
    /// It starts now: the caller runs [`App::run_ai_work`] off its own thread.
    Run,
    /// The same work, told to stop, is still winding down; this runs once it has.
    Waiting,
    /// The same work is already running or waiting.
    Duplicate,
    /// No host is installed, so nothing can be sent; the work was dropped.
    NoHost,
}

/// The work running, the work started that no runner has taken yet, and the work waiting for a
/// stopped run under its own key to wind down. At most one piece of work runs under a key.
pub(crate) struct Queue<P> {
    /// In the order it started.
    running: Vec<(AiWorkKey, AiCancel)>,
    started: VecDeque<(AiWork<P>, AiCancel)>,
    waiting: VecDeque<AiWork<P>>,
}

impl<P> Default for Queue<P> {
    fn default() -> Self {
        Self {
            running: Vec::new(),
            started: VecDeque::new(),
            waiting: VecDeque::new(),
        }
    }
}

impl<P> Queue<P> {
    /// Whether `key`'s work is running or waiting. Work told to stop is no longer the same work,
    /// so asking again while it winds down queues it afresh.
    fn holds(&self, key: &AiWorkKey) -> bool {
        self.live(key).is_some() || self.waiting.iter().any(|work| work.key == *key)
    }

    /// The stop of the work running under `key` that has not been told to stop.
    fn live(&self, key: &AiWorkKey) -> Option<&AiCancel> {
        self.running
            .iter()
            .find(|(running, cancel)| running == key && !cancel.is_set())
            .map(|(_, cancel)| cancel)
    }

    /// Marks the work under `key` running, and answers its stop.
    fn start(&mut self, key: AiWorkKey) -> AiCancel {
        let cancel = AiCancel::default();
        self.running.push((key, cancel.clone()));
        cancel
    }

    /// Ends the run under `key`, and starts the work waiting for it, if any.
    fn finish(&mut self, key: &AiWorkKey) -> Option<(AiWork<P>, AiCancel)> {
        self.running.retain(|(running, _)| running != key);
        let at = self.waiting.iter().position(|work| work.key == *key)?;
        let work = self.waiting.remove(at)?;
        let cancel = self.start(work.key.clone());
        Some((work, cancel))
    }
}

impl<P: Provider> App<P> {
    fn ai_queue(&self) -> std::sync::MutexGuard<'_, Queue<P>> {
        self.ai.work.lock().expect("ai work mutex poisoned")
    }

    /// Queues `work` and signals its surface. Returns at once; on [`AiQueued::Run`] the caller
    /// runs [`App::run_ai_work`] off its own thread.
    pub fn queue_ai_work(&self, work: AiWork<P>) -> AiQueued {
        if self.ai.ports().host.is_none() {
            log::warn!("ai: work was asked for with nowhere to run it");
            return AiQueued::NoHost;
        }
        let surface = work.key.surface();
        let queued = {
            let mut queue = self.ai_queue();
            if queue.holds(&work.key) {
                return AiQueued::Duplicate;
            }
            if queue
                .running
                .iter()
                .any(|(running, _)| *running == work.key)
            {
                queue.waiting.push_back(work);
                AiQueued::Waiting
            } else {
                let cancel = queue.start(work.key.clone());
                queue.started.push_back((work, cancel));
                AiQueued::Run
            }
        };
        self.observer.surface_changed(surface);
        queued
    }

    /// Runs one piece of work [`App::queue_ai_work`] started, then the work under the same key
    /// that waited for it, each to its end. Not for a UI thread.
    pub fn run_ai_work(&self) {
        self.run_started(|started| started.pop_front());
    }

    /// Runs the started work `take` takes, then the work under its key that waited for it.
    fn run_started(
        &self,
        take: impl FnOnce(&mut VecDeque<(AiWork<P>, AiCancel)>) -> Option<(AiWork<P>, AiCancel)>,
    ) {
        let Some(host) = self.ai.ports().host.clone() else {
            return;
        };
        let mut next = take(&mut self.ai_queue().started);
        while let Some((work, cancel)) = next {
            let key = work.key.clone();
            let run = AiRun {
                cancel,
                host: host.as_ref(),
            };
            (work.body)(self, &run);
            next = self.ai_queue().finish(&key);
            self.observer.surface_changed(key.surface());
        }
    }

    /// Queues `work` and runs it on this thread: for tests and for a host with a thread to spare.
    /// Not for a UI thread. Takes this work rather than whichever started first, since another
    /// runner may not have taken its own yet, and that work may wait on this thread's caller.
    pub fn run_ai_work_now(&self, work: AiWork<P>) {
        let key = work.key.clone();
        if self.queue_ai_work(work) == AiQueued::Run {
            self.run_started(|started| {
                let at = started.iter().position(|(work, _)| work.key == key)?;
                started.remove(at)
            });
        }
    }

    /// Stops the work under `key`: told to stop while it runs, taken out of the queue while it
    /// waits. Whether there was such work.
    pub fn cancel_ai_work(&self, key: &AiWorkKey) -> bool {
        let stopped = {
            let mut queue = self.ai_queue();
            if let Some(cancel) = queue.live(key) {
                cancel.set();
                true
            } else {
                let before = queue.waiting.len();
                queue.waiting.retain(|work| work.key != *key);
                queue.waiting.len() != before
            }
        };
        if stopped {
            log::info!("ai: work was stopped");
            self.observer.surface_changed(key.surface());
        }
        stopped
    }

    /// The work running, in the order it started, then the work waiting. Work told to stop is
    /// not listed while it winds down.
    #[must_use]
    pub fn ai_work(&self) -> Vec<AiWorkKey> {
        let queue = self.ai_queue();
        queue
            .running
            .iter()
            .filter(|(_, cancel)| !cancel.is_set())
            .map(|(key, _)| key.clone())
            .chain(queue.waiting.iter().map(|work| work.key.clone()))
            .collect()
    }

    /// Whether the work under `key` is running now, rather than waiting, stopped or not queued.
    #[must_use]
    pub fn ai_work_running(&self, key: &AiWorkKey) -> bool {
        self.ai_queue().live(key).is_some()
    }
}
