//! `allodia-mcp`: the stdio relay an MCP client spawns to reach a running Allodia Mail &
//! Calendar.
//!
//! # Why a relay at all
//!
//! An MCP client spawns its server as a child process and talks to it over stdio. But the
//! **running app** owns `mailcal.sqlite`, the live IMAP `IDLE` connections and the in-memory
//! credentials. A standalone server process would mean two writers on one SQLite file and a
//! second copy of the user's secrets. This resolves that: the client gets the stdio it expects,
//! the app stays the single owner, and what crosses between them is bytes.
//!
//! # Why not loopback HTTP with a bearer token
//!
//! Because then a secret exists. A token has to be generated, stored, shown to the user, pasted
//! into a config file, rotated, and kept out of backups, and it would be *load-bearing*, since
//! a permanent listener on a port is reachable by any local process. Over a Unix socket in a
//! 0700 directory (or a named pipe with remote clients rejected), the OS user boundary does the
//! authenticating and there is nothing to leak. Framing helps too: stdio's has been unchanged
//! since 2024-11 while Streamable HTTP is the transport that churns.
//!
//! # Two behaviours that break every client if wrong
//!
//! 1. **Nothing but relayed frames ever reaches stdout.** Diagnostics go to stderr only. This is an
//!    MCP stdio contract, not a preference; one stray `println!` and the client's parser
//!    desynchronizes and the server looks broken.
//! 2. **A failed connect is answered, not crashed.** The app may simply not be running. Exiting
//!    makes the client report a broken server; replying with a JSON-RPC error and staying alive
//!    makes it report an *unavailable* one, which is both true and recoverable the moment the user
//!    opens the app.
//!
//! # One relay, two transports
//!
//! The endpoint is a Unix domain socket or a Windows named pipe, and the difference is **not**
//! cosmetic: the two need different I/O models, for a reason that cost a day (see the `windows`
//! module). Each lives in its own module so neither carries the other's shape; everything above is
//! true of both, and [`frame`] holds the parts that are pure.
//!
//! Those two modules are `cfg`-gated, so a doc **link** to either resolves on only one host and
//! fails rustdoc on the other; hence the plain code spans.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

mod frame;

use std::process::ExitCode;

#[cfg(unix)]
use crate::unix::relay;
#[cfg(windows)]
use crate::windows::relay;

fn main() -> ExitCode {
    let Some(endpoint) = endpoint_argument() else {
        eprintln!(
            "allodia-mcp: usage: allodia-mcp --endpoint <socket path or pipe name>\n\
             Allodia Mail & Calendar generates the exact value in Settings → Advanced."
        );
        return ExitCode::FAILURE;
    };
    relay(&endpoint);
    ExitCode::SUCCESS
}

/// Reads `--endpoint <value>` (or `--endpoint=<value>`) from the command line.
///
/// The endpoint is **passed in**, never derived here. That is deliberate: this binary and the
/// app would otherwise each compute a path, and the day one of them changed: a sandboxed build,
/// a dev suffix, a different data directory; they would disagree silently and the relay would
/// connect to nothing. The Settings screen generates the exact string that goes in the client's
/// config, so there is one derivation, in the one place that knows the answer.
fn endpoint_argument() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--endpoint=") {
            return Some(value.to_owned());
        }
        if arg == "--endpoint" {
            return args.next();
        }
    }
    None
}
