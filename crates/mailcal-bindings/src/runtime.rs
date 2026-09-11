// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! The async runtime every FFI call is driven on.

use tokio::runtime::{Builder, Runtime};

/// Builds the app's async runtime, capping worker threads to **one fewer than the core
/// count** so a heavy multi-folder sync never saturates every core and starves the host's
/// UI thread (the user's "keep the UI thread on its own core" ask). At least one worker.
pub(crate) fn build_runtime() -> std::io::Result<Runtime> {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let workers = cores.saturating_sub(1).max(1);
    Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
}

/// The capped runtime, panicking if it cannot start (the demo path, where a missing
/// runtime is fatal anyway).
pub(crate) fn runtime() -> Runtime {
    build_runtime().expect("tokio runtime starts")
}
