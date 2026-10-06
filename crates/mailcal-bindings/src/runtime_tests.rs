// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! A blocking FFI call is polled on a thread of our own, whatever thread the host calls it from.

use std::hint::black_box;

use super::{DRIVER_NAME, drive, runtime};

/// The stack a host thread might have: well under what [`deep`] needs.
const HOST_STACK: usize = 512 * 1024;

/// Uses about `frames` × 64 KiB of stack, the way a deep connect does in a debug build.
fn deep(frames: usize) -> usize {
    let frame = [1u8; 64 * 1024];
    let here = black_box(&frame)
        .iter()
        .map(|&b| usize::from(b))
        .sum::<usize>();
    if frames == 0 {
        here
    } else {
        here + deep(frames - 1)
    }
}

/// Runs `call` on a thread with [`HOST_STACK`], as a Swift concurrency thread would.
fn from_a_host_thread<T: Send + 'static>(call: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(HOST_STACK)
        .spawn(call)
        .expect("a host thread starts")
        .join()
        .expect("the host thread returns")
}

#[test]
fn a_call_is_polled_on_the_driver_thread_not_the_callers() {
    let runtime = runtime();
    let name = from_a_host_thread(move || {
        runtime.block_on(async { std::thread::current().name().map(str::to_owned) })
    });
    assert_eq!(name.as_deref(), Some(DRIVER_NAME));
}

#[test]
fn work_deeper_than_the_callers_stack_completes() {
    // About 2 MiB, four times the host thread's stack. Polled on the caller, this ends the test
    // process with a stack overflow rather than failing an assertion.
    let runtime = runtime();
    let used = from_a_host_thread(move || runtime.block_on(async { deep(32) }));
    assert_eq!(used, 33 * 64 * 1024);
}

#[test]
fn a_panic_in_the_call_reaches_the_caller() {
    let runtime = runtime();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.block_on(async { panic!("the provider panicked") })
    }));
    let payload = outcome.expect_err("the panic resumes on the caller");
    assert_eq!(
        payload.downcast_ref::<&str>(),
        Some(&"the provider panicked")
    );
}

#[test]
fn a_call_from_a_runtime_worker_still_completes() {
    // A scheduled pass reaches the bindings from a worker: waiting there must not take the
    // worker the call itself needs.
    let runtime = runtime();
    let handle = runtime.handle().clone();
    let answer = drive(runtime.handle(), async move {
        tokio::spawn(async move { drive(&handle, async { 42 }) })
            .await
            .expect("the worker task completes")
    });
    assert_eq!(answer, 42);
}
