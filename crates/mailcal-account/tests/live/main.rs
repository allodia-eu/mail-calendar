//! The live suites, against the local harness or a real provider, as one test binary.
//!
//! Each module skips unless the variables its header names are set, so an offline `cargo test`
//! stays green; `scripts/dev/harness.sh test` sets them and runs one module at a time, as
//! `cargo test --test live -- <module>::`.
//!
//! One binary rather than one per suite: each integration test file is a crate of its own that
//! links the whole account layer, so every suite was one more link of it in every test build.
//! Nothing here installs a process-global logger, so the suites can share a process;
//! `live_graph_throttle.rs`, which does, stays a binary of its own.

mod live_google;
mod live_google_affiliation;
mod live_graph_affiliation;
mod live_graph_contacts;
mod live_imap_auth;
mod live_jmap;
