//! The Unix relay: a Unix domain socket, two threads, and no dependencies at all.
//!
//! A socket's duplicated file descriptor supports a read and a write **at the same time**, so the
//! whole program is a blocking read loop on stdin plus a thread copying the socket to stdout.
//! `std` alone is enough. (Windows cannot do this; see the crate's `windows` module, which is
//! `cfg`-gated and so cannot be a doc link from here.)

use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    sync::{Arc, Mutex},
};

use crate::frame::unavailable_frame;

/// The relay loop: every line from stdin goes to the app, everything the app says goes to stdout.
pub(crate) fn relay(endpoint: &str) {
    // Shared because the reader thread and this loop's error path both write to it, and an
    // interleaved half-line would desynchronize the client's parser.
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let mut connection: Option<UnixStream> = None;
    let stdin = std::io::stdin();
    for line in BufReader::new(stdin.lock()).lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if connection.is_none() {
            match UnixStream::connect(endpoint) {
                Ok(stream) => {
                    start_reader(&stream, &out);
                    connection = Some(stream);
                }
                Err(err) => {
                    eprintln!("allodia-mcp: could not reach the app: {err}");
                    write_line(&out, &unavailable_frame(&line));
                    continue;
                }
            }
        }
        let Some(stream) = connection.as_mut() else {
            continue;
        };
        if writeln!(stream, "{line}").is_err() || stream.flush().is_err() {
            // The app went away mid-session (quit, or restarted). Drop the connection so the
            // next request re-dials, and answer this one rather than dropping it on the floor.
            eprintln!("allodia-mcp: the app closed the connection");
            connection = None;
            write_line(&out, &unavailable_frame(&line));
        }
    }
}

/// Spawns the socket-to-stdout pump for one connection.
fn start_reader(stream: &UnixStream, out: &Arc<Mutex<std::io::Stdout>>) {
    let Ok(reader) = stream.try_clone() else {
        eprintln!("allodia-mcp: could not split the connection");
        return;
    };
    let out = Arc::clone(out);
    std::thread::spawn(move || pump(reader, &out));
}

/// Copies everything the app sends to stdout, verbatim, until the connection closes.
fn pump<R: Read>(reader: R, out: &Arc<Mutex<std::io::Stdout>>) {
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => write_line(out, line.trim_end_matches(['\r', '\n'])),
        }
    }
}

/// Writes one frame to stdout and flushes. Flushing every frame is required: an MCP client reads
/// line by line and a buffered response is a hang, not a delay.
fn write_line(out: &Arc<Mutex<std::io::Stdout>>, body: &str) {
    let Ok(mut out) = out.lock() else {
        return;
    };
    let _ = writeln!(out, "{body}");
    let _ = out.flush();
}
