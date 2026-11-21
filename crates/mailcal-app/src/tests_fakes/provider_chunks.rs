//! How [`super::FakeProvider`] hands its messages over: in the chunks a real adapter yields.

use engine_core::{mail::Message, sync::SyncState};
use engine_provider::EmailChunk;

/// Splits `messages` the way a real adapter yields them: `chunk_size` per chunk, `0` meaning
/// one chunk for the lot (what [`StreamTuning`](engine_api::StreamTuning) documents).
///
/// A fake that hands the whole mailbox over in one chunk cannot see the core's commit
/// granularity at all, and that granularity is what a streamed pass costs, because the core
/// re-projects its cached list once per commit. So the parameter under test is honoured here
/// rather than ignored.
///
/// An empty mailbox still yields one (empty) chunk: a pass that finds nothing still has a final
/// chunk to reconcile the store against.
pub(super) fn pages(messages: &[Message], chunk_size: usize) -> impl Iterator<Item = &[Message]> {
    let pages: Vec<&[Message]> = match chunk_size {
        _ if messages.is_empty() => vec![&[]],
        0 => vec![messages],
        size => messages.chunks(size).collect(),
    };
    pages.into_iter()
}

/// `messages` as a reconciling pass's chunks: intermediate pages hold the cursor and carry the
/// keys they cover, and only the last advances it and tombstones what no page named.
pub(super) fn reconcile_chunks(messages: &[Message], chunk_size: usize) -> Vec<EmailChunk> {
    let total = messages.len();
    let pages: Vec<&[Message]> = pages(messages, chunk_size).collect();
    let last = pages.len() - 1;
    pages
        .into_iter()
        .enumerate()
        .map(|(index, page)| {
            let present = page.iter().map(|m| m.id.key().clone()).collect();
            if index == last {
                EmailChunk::reconcile_last(
                    page.to_vec(),
                    present,
                    Some(total),
                    SyncState::new("email-1"),
                )
            } else {
                EmailChunk::reconcile_page(page.to_vec(), present, Some(total))
            }
        })
        .collect()
}
