//! The Windows relay: a named pipe, driven with **overlapped** I/O.
//!
//! # Why this is not a copy of the Unix module, with `UnixStream` swapped out
//!
//! It was, and it was broken in a way no test on this repo could see. A named pipe opened by
//! `File::open` is a **synchronous** file object, and Windows serializes I/O on a synchronous file
//! object: a blocking `ReadFile` holds it, and a concurrent `WriteFile`; even on a *duplicated*
//! handle, because `DuplicateHandle` yields another handle to the *same* object; queues behind
//! that read until it completes. So the reader thread, parked waiting for the app to say
//! something, blocks the writer thread from ever telling it anything. Deadlock.
//!
//! A Unix socket has no such rule, which is why the same code is correct there and why this went
//! unnoticed: the relay's own suite is `#![cfg(unix)]`, and every test in it sent exactly **one**
//! request: the one request that works. The observed symptom was that `initialize` was answered
//! and `tools/list` hung forever, which reads as a broken *server* rather than a broken relay.
//!
//! The fix is the standard one: an overlapped handle, so a read in flight does not exclude a
//! write. Getting that safely from Rust means `tokio`'s named-pipe client, which is why **this
//! target, and only this target, has a dependency at all** (see `Cargo.toml`). The doctrine that
//! bought the zero-dependency rule is untouched: this binary still links nothing of the mail
//! stack, and Unix stays literally dependency-free.
//!
//! Note what this deliberately is *not*: a lockstep "write a request, then read exactly one
//! reply" loop. That would also avoid the overlap, with no dependency, by relying on the server
//! never speaking first. It is true today and it is not a property to build on: the server's
//! whole point is that the user's settings can change under a live connection, and the day it
//! grows a `notifications/tools/list_changed` the relay would hang instead of forwarding it.

use std::io::{self, BufRead, Write};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf},
    net::windows::named_pipe::{ClientOptions, NamedPipeClient},
    sync::mpsc,
};

use crate::frame::unavailable_frame;

/// How many frames may be queued on either channel before its producer waits. Only ever holds
/// lines not yet acted on; a bound keeps a wedged consumer from growing memory forever.
const FRAME_QUEUE: usize = 64;

/// `ERROR_PIPE_BUSY`. Every instance of the pipe is currently connected: the server has accepted
/// ours and has not finished creating the next one yet. A tiny, self-clearing window, and worth
/// retrying rather than reporting the app as unavailable.
const ERROR_PIPE_BUSY: i32 = 231;

/// How many times, and how long apart, to retry a busy pipe. Bounded: the point is to ride out an
/// instance being replaced, never to sit waiting for an app that is not running.
const BUSY_RETRIES: u32 = 10;
const BUSY_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(50);

/// The relay loop: every line from stdin goes to the app, everything the app says goes to stdout.
pub(crate) fn relay(endpoint: &str) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            // stderr, never stdout: a diagnostic on stdout desynchronizes the client's parser.
            eprintln!("allodia-mcp: could not start the I/O runtime: {err}");
            return;
        }
    };
    runtime.block_on(serve(endpoint));
}

/// Reads stdin and the app's replies concurrently until stdin closes.
///
/// **Both arms are channels, and neither is an I/O future.** `select!` drops whichever branch did
/// not win, so anything polled directly here has to be cancel-safe, and `tokio::io::stdin()` is
/// not: it is a blocking read on a helper thread, and a cancelled one can lose what it had already
/// taken. That is not theoretical. The first cut of this fix polled it directly and swallowed the
/// request after every reply, turning one deadlock into another that looked identical from the
/// client. An `mpsc::Receiver::recv()` *is* cancel-safe, so stdin gets a dedicated thread and the
/// loop only ever selects over queues.
async fn serve(endpoint: &str) {
    let mut input = read_stdin_on_its_own_thread();
    // The app's replies likewise arrive on a channel rather than being read inline, so the two
    // directions are independent: a reply reaches stdout while the next request is being written.
    let (sender, mut replies) = mpsc::channel::<String>(FRAME_QUEUE);
    let mut app: Option<WriteHalf<NamedPipeClient>> = None;

    loop {
        tokio::select! {
            line = input.recv() => {
                // stdin closed: the client is done with us.
                let Some(line) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                if app.is_none() {
                    match connect(endpoint, sender.clone()).await {
                        Ok(writer) => app = Some(writer),
                        Err(err) => {
                            eprintln!("allodia-mcp: could not reach the app: {err}");
                            write_line(&unavailable_frame(&line));
                            continue;
                        }
                    }
                }
                let Some(writer) = app.as_mut() else { continue };
                if writer.write_all(line.as_bytes()).await.is_err()
                    || writer.write_all(b"\n").await.is_err()
                    || writer.flush().await.is_err()
                {
                    // The app went away mid-session (quit, or restarted). Drop the connection so
                    // the next request re-dials, and answer this one rather than dropping it.
                    eprintln!("allodia-mcp: the app closed the connection");
                    app = None;
                    write_line(&unavailable_frame(&line));
                }
            }
            // `sender` is held above for the life of this loop, so this never resolves to `None`
            // and the branch cannot go permanently dead when a connection ends.
            Some(frame) = replies.recv() => write_line(&frame),
        }
    }
}

/// Pumps stdin onto a channel from a dedicated OS thread.
///
/// Blocking `std` reads on purpose; see [`serve`]. The thread ends when stdin closes, dropping
/// the sender, which is how the loop learns to stop.
fn read_stdin_on_its_own_thread() -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel::<String>(FRAME_QUEUE);
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if sender.blocking_send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

/// Opens the pipe and starts pumping its replies onto `replies`, returning the write half.
///
/// # Errors
///
/// Whatever `CreateFile` reported, after riding out a transient `ERROR_PIPE_BUSY`.
async fn connect(
    endpoint: &str,
    replies: mpsc::Sender<String>,
) -> io::Result<WriteHalf<NamedPipeClient>> {
    let mut attempt = 0;
    let client = loop {
        match ClientOptions::new().open(endpoint) {
            Ok(client) => break client,
            Err(err) if err.raw_os_error() == Some(ERROR_PIPE_BUSY) && attempt < BUSY_RETRIES => {
                attempt += 1;
                tokio::time::sleep(BUSY_RETRY_DELAY).await;
            }
            Err(err) => return Err(err),
        }
    };
    let (reader, writer) = tokio::io::split(client);
    tokio::spawn(pump(reader, replies));
    Ok(writer)
}

/// Copies everything the app sends onto `replies`, verbatim and one frame per line, until the
/// connection closes.
async fn pump(reader: ReadHalf<NamedPipeClient>, replies: mpsc::Sender<String>) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if replies.send(line).await.is_err() {
            break;
        }
    }
}

/// Writes one frame to stdout and flushes. Flushing every frame is required: an MCP client reads
/// line by line and a buffered response is a hang, not a delay.
///
/// Plain `std`, called from the loop rather than awaited: a frame is a few hundred bytes to a pipe
/// this process owns, and there is nothing else for the relay to be doing meanwhile.
fn write_line(body: &str) {
    let mut output = io::stdout().lock();
    let _ = writeln!(output, "{body}");
    let _ = output.flush();
}
