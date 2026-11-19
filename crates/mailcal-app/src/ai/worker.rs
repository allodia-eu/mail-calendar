//! The work that sends to a model: learning a style and drafting a reply (`docs/ai.md` rules 13
//! and 20), one piece at a time, off the host's thread.
//!
//! A piece of work is queued on the host's thread and returns at once; the caller that was told to
//! runs the queue on a thread of its own. Work asked for again while the same work runs or waits
//! is dropped, so a double press starts it once. Stopping work that waits takes it out of the
//! queue; stopping work that runs sets its [`AiCancel`], which abandons the request in flight and
//! which the work reads before its next. What a piece of work came to is its own surface's to
//! record: the queue only says what runs and what waits, and signals the surface it belongs to as
//! that moves.

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
    /// Nothing else was running: the caller runs the queue, off its own thread, with
    /// [`App::run_ai_work`].
    Run,
    /// It runs after the work running now.
    Waiting,
    /// The same work is already running or waiting.
    Duplicate,
    /// No host is installed, so nothing can be sent; the work was dropped.
    NoHost,
}

/// The work running, and the work waiting behind it.
pub(crate) struct Queue<P> {
    running: Option<(AiWorkKey, AiCancel)>,
    /// The running work's body, until the runner takes it.
    next: Option<AiWork<P>>,
    waiting: VecDeque<AiWork<P>>,
}

impl<P> Default for Queue<P> {
    fn default() -> Self {
        Self {
            running: None,
            next: None,
            waiting: VecDeque::new(),
        }
    }
}

impl<P> Queue<P> {
    /// Whether `key`'s work is running or waiting. Work told to stop is no longer the same work,
    /// so asking again while it winds down queues it afresh.
    fn holds(&self, key: &AiWorkKey) -> bool {
        self.running
            .as_ref()
            .is_some_and(|(running, cancel)| running == key && !cancel.is_set())
            || self.waiting.iter().any(|work| work.key == *key)
    }

    /// Starts the next work waiting, if any.
    fn advance(&mut self) {
        self.running = None;
        if let Some(work) = self.waiting.pop_front() {
            self.running = Some((work.key.clone(), AiCancel::default()));
            self.next = Some(work);
        }
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
            if queue.running.is_some() {
                queue.waiting.push_back(work);
                AiQueued::Waiting
            } else {
                queue.running = Some((work.key.clone(), AiCancel::default()));
                queue.next = Some(work);
                AiQueued::Run
            }
        };
        self.observer.surface_changed(surface);
        queued
    }

    /// Runs the work [`App::queue_ai_work`] started, and all that is queued behind it, each to its
    /// end. Not for a UI thread.
    pub fn run_ai_work(&self) {
        let Some(host) = self.ai.ports().host.clone() else {
            return;
        };
        loop {
            let (work, cancel) = {
                let mut queue = self.ai_queue();
                let Some(work) = queue.next.take() else {
                    return;
                };
                let cancel = queue
                    .running
                    .as_ref()
                    .map(|(_, cancel)| cancel.clone())
                    .unwrap_or_default();
                (work, cancel)
            };
            let surface = work.key.surface();
            let run = AiRun {
                cancel,
                host: host.as_ref(),
            };
            (work.body)(self, &run);
            let next = {
                let mut queue = self.ai_queue();
                queue.advance();
                queue.running.as_ref().map(|(key, _)| key.surface())
            };
            self.observer.surface_changed(surface);
            if let Some(next) = next.filter(|next| *next != surface) {
                self.observer.surface_changed(next);
            }
        }
    }

    /// Queues `work` and runs it on this thread: for tests and for a host with a thread to spare.
    /// Not for a UI thread.
    pub fn run_ai_work_now(&self, work: AiWork<P>) {
        if self.queue_ai_work(work) == AiQueued::Run {
            self.run_ai_work();
        }
    }

    /// Stops the work under `key`: taken out of the queue while it waits, told to stop while it
    /// runs. Whether there was such work.
    pub fn cancel_ai_work(&self, key: &AiWorkKey) -> bool {
        let stopped = {
            let mut queue = self.ai_queue();
            if let Some((_, cancel)) = queue.running.as_ref().filter(|(running, _)| running == key)
            {
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

    /// The work running, then the work waiting behind it. Work told to stop is not listed while
    /// it winds down.
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
        self.ai_queue()
            .running
            .as_ref()
            .is_some_and(|(running, cancel)| running == key && !cancel.is_set())
    }
}
