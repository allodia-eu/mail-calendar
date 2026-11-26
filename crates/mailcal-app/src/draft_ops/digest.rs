//! What counts as an unchanged draft: the digest a save compares against the last one, so
//! pressing "Save as draft" twice reaches no server (`docs/drafts.md`).
//!
//! A child of [`super`], whose `impl App` block it does not continue: these are free
//! functions, split out so that file stays under the 500-line limit.

use std::{
    hash::{DefaultHasher, Hasher},
    io,
};

use engine_api::{Draft, MessageMetadata};

/// A digest of everything a save would put on the server: the draft and the metadata asked for
/// on it.
///
/// Over the serialized draft and metadata rather than a chosen subset of their fields: a subset
/// is a list that goes stale the moment either grows a field, and the failure is silent, an edit
/// the user made that never leaves the device.
///
/// Streamed into the hasher rather than serialized to a buffer first. A draft carries its
/// attachments' bytes, and JSON writes each byte as a decimal number, so buffering a message
/// with a 20 MB file costs some 70 MB of allocation on every idle save.
pub(super) fn digest(draft: &Draft, metadata: &MessageMetadata) -> u64 {
    let mut sink = HashSink(DefaultHasher::new());
    if serde_json::to_writer(&mut sink, draft).is_err()
        || serde_json::to_writer(&mut sink, metadata).is_err()
    {
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

#[cfg(test)]
mod tests {
    use engine_api::{Draft, EmailAddress, MessageIdHeader, MessageMetadata};
    use mailcal_account::drafted_reply_keyword;

    use super::digest;

    fn draft() -> Draft {
        Draft::new(
            MessageIdHeader::new("draft@example.com").unwrap(),
            EmailAddress::new("me@example.com"),
            vec![EmailAddress::new("you@example.com")],
            "Plans",
            "Friday works.",
        )
    }

    #[test]
    fn an_unchanged_draft_has_the_digest_of_its_last_save() {
        let marked = MessageMetadata::default().with_keyword(drafted_reply_keyword());
        assert_eq!(digest(&draft(), &marked), digest(&draft(), &marked.clone()));
    }

    /// A mark put on or taken off is a change the server has to hear about.
    #[test]
    fn the_metadata_asked_for_is_part_of_the_digest() {
        let marked = MessageMetadata::default().with_keyword(drafted_reply_keyword());
        assert_ne!(
            digest(&draft(), &MessageMetadata::default()),
            digest(&draft(), &marked)
        );
    }
}
