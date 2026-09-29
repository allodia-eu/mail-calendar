//! Driving a future on the app's runtime from a synchronous caller: the shape every blocking
//! port in this crate shares (the Allodia account service's, and the AI endpoint's).

/// Drive one future to completion on the app's runtime, from a synchronous caller.
///
/// A pass runs on the thread that asked for it (a host's background thread) where handing the
/// future to the runtime is all there is to it. A pass started *from* the runtime, which a
/// scheduled one would be, is on a worker instead, and parking that worker is what
/// [`block_in_place`](tokio::task::block_in_place) exists to avoid: it moves the thread out of the
/// scheduler first, so the remaining work still has somewhere to run.
pub(crate) fn block_on<T>(handle: &tokio::runtime::Handle, future: impl Future<Output = T>) -> T {
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        handle.block_on(future)
    }
}
