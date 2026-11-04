//! The host's DNS as detection used it, for a caller that keeps what detection found.

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// The host's DNS, watched: whether every lookup detection made through it answered.
///
/// Detection never says that a lookup failed. It carries on without the answer, to the probes that
/// need no DNS, so what it returns while DNS is failing is what a lookup without DNS returns.
/// A lookup it gave up waiting for is still running when it returns, and counts as unanswered.
pub(crate) struct WatchedResolver {
    inner: Arc<dyn mailcal_autodetect::MxResolver>,
    started: AtomicUsize,
    finished: AtomicUsize,
    failed: AtomicBool,
    mx_answered: AtomicBool,
}

impl WatchedResolver {
    pub(crate) fn over(inner: Arc<dyn mailcal_autodetect::MxResolver>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            started: AtomicUsize::new(0),
            finished: AtomicUsize::new(0),
            failed: AtomicBool::new(false),
            mx_answered: AtomicBool::new(false),
        })
    }

    /// Whether an MX lookup answered, none failed, and none is still waiting for an answer.
    pub(crate) fn answered_every_lookup(&self) -> bool {
        self.mx_answered.load(Ordering::SeqCst)
            && !self.failed.load(Ordering::SeqCst)
            && self.started.load(Ordering::SeqCst) == self.finished.load(Ordering::SeqCst)
    }

    fn watch<T>(
        &self,
        lookup: impl FnOnce() -> Result<T, mailcal_autodetect::MxError>,
    ) -> Result<T, mailcal_autodetect::MxError> {
        self.started.fetch_add(1, Ordering::SeqCst);
        let result = lookup();
        if result.is_err() {
            self.failed.store(true, Ordering::SeqCst);
        }
        self.finished.fetch_add(1, Ordering::SeqCst);
        result
    }
}

impl mailcal_autodetect::MxResolver for WatchedResolver {
    fn resolve_mx(
        &self,
        domain: &str,
    ) -> Result<mailcal_autodetect::MxResolution, mailcal_autodetect::MxError> {
        let result = self.watch(|| self.inner.resolve_mx(domain));
        if result.is_ok() {
            self.mx_answered.store(true, Ordering::SeqCst);
        }
        result
    }

    fn resolve_srv(
        &self,
        name: &str,
    ) -> Result<mailcal_autodetect::SrvResolution, mailcal_autodetect::MxError> {
        self.watch(|| self.inner.resolve_srv(name))
    }
}
