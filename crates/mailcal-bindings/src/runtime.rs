// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! The async runtime every FFI call is driven on, and the thread that drives it.
//!
//! A blocking FFI method never polls its future on the thread that called it. That thread is the
//! host's, and so is its stack size: a secondary thread on Apple platforms, a Swift concurrency
//! thread included, has 512 KiB, which a debug build's account connect overflows, ending the
//! process with a bus error. The future is polled on a thread of our own instead, sized by
//! [`DRIVER_STACK`], and the caller only waits for it. Whatever thread a host calls from, the
//! stack the core runs on is the same.
//!
//! `crates/mailcal-bindings/clippy.toml` forbids tokio's own `block_on`, so a call site cannot
//! go back to polling on the caller's stack: [`FfiRuntime::block_on`] and [`drive`] are the only
//! way to wait on a future here.

use std::future::Future;

use tokio::runtime::{Builder, Handle, Runtime, RuntimeFlavor};

/// The stack of the thread a blocking FFI call is polled on. An account connect is the deepest
/// path (dial, provider connect, HTTP client, TLS), and a debug build's futures are several times
/// the size of a release build's. The memory is reserved, not committed: a thread touches only
/// the pages it uses.
const DRIVER_STACK: usize = 16 * 1024 * 1024;

/// The stack of each runtime worker, which runs the same connects for boot and reconnect.
/// tokio's default is 2 MiB.
const WORKER_STACK: usize = 8 * 1024 * 1024;

/// The name of the thread [`drive`] polls on, as a crash report or a debugger shows it.
pub(crate) const DRIVER_NAME: &str = "mailcal-ffi-call";

/// The app's runtime, with the only `block_on` the bindings may call.
pub(crate) struct FfiRuntime {
    runtime: Runtime,
}

impl FfiRuntime {
    /// Runs `future` to completion on a thread of our own and returns its output; see the
    /// module docs.
    pub(crate) fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future + Send,
        F::Output: Send,
    {
        drive(self.runtime.handle(), future)
    }

    pub(crate) fn handle(&self) -> &Handle {
        self.runtime.handle()
    }

    pub(crate) fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(future)
    }

    pub(crate) fn enter(&self) -> tokio::runtime::EnterGuard<'_> {
        self.runtime.enter()
    }
}

/// Polls `future` on `handle`'s runtime from a new [`DRIVER_STACK`]-sized thread and waits for
/// it. A panic in the future resumes on the caller, where UniFFI turns it into an error.
///
/// Called from a runtime worker (a scheduled pass), the wait moves the worker out of the
/// scheduler first ([`tokio::task::block_in_place`]), so the work it waits for still has
/// somewhere to run.
#[allow(clippy::disallowed_methods)] // the one place tokio's `block_on` is called
pub(crate) fn drive<F>(handle: &Handle, future: F) -> F::Output
where
    F: Future + Send,
    F::Output: Send,
{
    std::thread::scope(|scope| {
        let driver = std::thread::Builder::new()
            .name(DRIVER_NAME.to_owned())
            .stack_size(DRIVER_STACK)
            .spawn_scoped(scope, || handle.block_on(future))
            .expect("the system starts a thread for an FFI call");
        let on_worker = Handle::try_current()
            .is_ok_and(|current| current.runtime_flavor() == RuntimeFlavor::MultiThread);
        let joined = if on_worker {
            tokio::task::block_in_place(|| driver.join())
        } else {
            driver.join()
        };
        joined.unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}

/// Builds the app's async runtime, capping worker threads to **one fewer than the core
/// count** so a heavy multi-folder sync never saturates every core and starves the host's
/// UI thread (the user's "keep the UI thread on its own core" ask). At least one worker.
pub(crate) fn build_runtime() -> std::io::Result<FfiRuntime> {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let workers = cores.saturating_sub(1).max(1);
    let runtime = Builder::new_multi_thread()
        .worker_threads(workers)
        .thread_stack_size(WORKER_STACK)
        .enable_all()
        .build()?;
    Ok(FfiRuntime { runtime })
}

/// The capped runtime, panicking if it cannot start (the demo path, where a missing
/// runtime is fatal anyway).
pub(crate) fn runtime() -> FfiRuntime {
    build_runtime().expect("tokio runtime starts")
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
