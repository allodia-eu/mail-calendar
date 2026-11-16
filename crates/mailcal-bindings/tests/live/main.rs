//! The live suites, against the local harness or a real provider, as one test binary.
//!
//! Each module skips unless the variables its header names are set, so an offline `cargo test`
//! stays green; `scripts/dev/harness.sh test` sets them and runs one module at a time, as
//! `cargo test --test live -- <module>::`.
//!
//! One binary rather than one per suite: each integration test file is a crate of its own that
//! links the whole core, so twelve of them were twelve links of it in every test build. Nothing
//! here installs a process-global logger or panic hook, so the suites can share a process; the
//! ones that do (`credential_logging.rs`, `panic_logging.rs`, `boot_skip_logging.rs`) stay
//! binaries of their own.

mod live_account_servers;
mod live_calendar_sign_in;
mod live_calendar_split;
mod live_imap_oauth;
mod live_jmap_oauth;
mod live_link_suggestions;
mod live_nextcloud_sign_in;
mod live_oauth;
mod live_provider_consent;
mod live_row_menu;
mod live_shared_calendars;
mod live_shared_mailboxes;
mod live_sign_in_switch;
