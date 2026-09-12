//! Who a reading-view snapshot is **for**.
//!
//! Most surfaces have one viewer, so one slot holds their snapshot. The reading body does not: a
//! desktop opens a message in a window of its own beside the pane, and the two show different
//! messages (`docs/reading-window.md`). So a body is stored per reader, and every reader is
//! opened through the same path: a window inherits the mark-read, the dial-wait retry and the
//! pending threshold rather than restating any of them.

/// One viewer of a message body.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReaderId {
    /// The reading pane, or the reading screen a phone pushes: the one viewer every client has.
    Pane,
    /// A detached reading window, named by the host, which mints one id per window and keeps it
    /// for as long as the window lives. The pane is a separate variant rather than a reserved
    /// name, so no host string can reach it.
    Window(String),
}
