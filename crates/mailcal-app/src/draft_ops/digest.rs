//! What counts as an unchanged draft: the digest a save compares against the last one, so
//! pressing "Save as draft" twice reaches no server (`docs/drafts.md`).
//!
//! A child of [`super`], whose `impl App` block it does not continue: these are free
//! functions, split out so that file stays under the 500-line limit.

use std::{
    hash::{DefaultHasher, Hasher},
    io,
};

use engine_api::Draft;

/// A digest of everything a save would put on the server.
///
/// Over the serialized draft rather than a chosen subset of its fields: a subset is a list
/// that goes stale the moment the draft grows a field, and the failure is silent, an edit the
/// user made that never leaves the device.
///
/// Streamed into the hasher rather than serialized to a buffer first. A draft carries its
/// attachments' bytes, and JSON writes each byte as a decimal number, so buffering a message
/// with a 20 MB file costs some 70 MB of allocation on every idle save.
pub(super) fn digest(draft: &Draft) -> u64 {
    let mut sink = HashSink(DefaultHasher::new());
    if serde_json::to_writer(&mut sink, draft).is_err() {
        return 0;
    }
    sink.0.finish()
}

/// An [`io::Write`] that hashes what is written to it and keeps none of it.
struct HashSink(DefaultHasher);

impl io::Write for HashSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
