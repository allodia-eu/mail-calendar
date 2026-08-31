// The face the reading header draws beside the sender while a message is still opening.
//
// The header fills from the row that was tapped, so the pane never opens on an empty circle, and
// upgrades to the body snapshot's avatar once that lands. A `pending` snapshot is neither: it
// exists to announce a wait and carries nothing else, so its avatar is the core's "nobody":
// empty initials over the colour for an empty address. Handing that to the header empties a
// circle the list had already filled, for exactly as long as the wait lasts.
//
// A pure function over the two, deliberately: it is the whole of the behaviour, and a unit test
// can pin it without composing a view or timing an open.

import MailcalBindings

/// The avatar for the reading header: the snapshot's once an open has actually resolved one,
/// otherwise the one the list row carried in.
///
/// `snapshot` is `nil` until something has been published for the open message and `pending` while
/// the open is still running; neither has resolved a face, so both answer `row`. Once a body
/// lands its avatar is the answer, the core resolved it against the same map the list read, so it
/// can only differ from the row's by having found a photo.
func readingHeaderAvatar(snapshot: ReadingSnapshot?, row: Avatar) -> Avatar {
    guard let snapshot, !snapshot.pending else { return row }
    return snapshot.avatar
}
