//! Finding the built relay, shared by the two transport suites.

use std::ffi::OsString;

/// The `allodia-mcp` binary this `cargo test` just built.
///
/// ⚠️ **`env!("CARGO_BIN_EXE_allodia-mcp")` is not this checkout's copy.** It is resolved when the
/// test is *compiled*, and every checkout shares one build directory
/// ([`.cargo/config.toml`](../../../../.cargo/config.toml)), while the binaries themselves are
/// uplifted into each worktree's own `target/`. A test binary compiled by one worktree is reused
/// by the next, carrying the first one's absolute path: the tests then spawn a relay belonging to
/// a different checkout, or, once that checkout is deleted, fail with `NotFound` in a tree whose
/// diff does not touch this crate. What it looks like from the outside is five tests failing over
/// a change that cannot have caused them.
///
/// Cargo exports the same variable into the test *process*, where it is computed by the run that
/// is happening now, so it always names the binary beside this run's. The compile-time value stays
/// as the fallback, for running the test executable directly rather than through cargo.
pub(crate) fn relay_binary() -> OsString {
    from_runtime(std::env::var_os("CARGO_BIN_EXE_allodia-mcp"))
}

/// [`relay_binary`] with the environment passed in, so the precedence can be tested without
/// writing to it: `std::env::set_var` is `unsafe` in this edition and the workspace forbids
/// `unsafe_code`.
fn from_runtime(runtime: Option<OsString>) -> OsString {
    runtime.unwrap_or_else(|| OsString::from(env!("CARGO_BIN_EXE_allodia-mcp")))
}

#[test]
fn the_running_cargo_decides_which_binary_is_spawned() {
    // The whole fix: what this run says, over what some earlier compile baked in.
    assert_eq!(
        from_runtime(Some(OsString::from("/from/this/run/allodia-mcp"))),
        OsString::from("/from/this/run/allodia-mcp")
    );
}

#[test]
fn without_cargo_the_compiled_path_still_answers() {
    // Running the test executable directly, where nothing exports the variable.
    assert_eq!(
        from_runtime(None),
        OsString::from(env!("CARGO_BIN_EXE_allodia-mcp"))
    );
}

#[test]
fn the_relay_this_run_built_is_on_disk() {
    // Named rather than left to `spawn`'s `NotFound`, which says only "No such file or directory"
    // and sends the reader into the relay rather than into the build.
    let binary = relay_binary();
    assert!(
        std::path::Path::new(&binary).exists(),
        "the relay this run built is not at {}: a stale path from another checkout is the usual \
         cause (see relay_binary)",
        std::path::Path::new(&binary).display()
    );
}
