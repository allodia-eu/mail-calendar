//! A draft the core asks the host to open in its composer.
//!
//! The one place the core initiates composing. Everywhere else the client opens its own
//! composer from a message it already has (a reply, a forward) and the core only hears about
//! it at submit time. Editing a **queued** send is different: the message exists nowhere the
//! client can read it, only inside an outbox op, so the core has to hand it over.
//!
//! Shaped like the standing question in [`unfiled_copy`](crate::unfiled_copy): the core puts
//! one here and signals [`Surface::ComposeRequest`](crate::Surface::ComposeRequest); the host
//! pulls it, opens its composer, and dismisses it. It does **not** auto-clear, because a
//! client that was backgrounded when the request was raised must still find it on return, and
//! the message it carries is the only copy left.

/// A message to open, unsent, in the host's composer.
///
/// The recipient fields are comma-joined rather than lists, matching the shape the composer
/// intents already take across the FFI: one representation of "a recipient field", not two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeRequest {
    /// The account to send from.
    pub account: String,
    /// The `To` field, comma-joined.
    pub to: String,
    /// The `Cc` field, comma-joined.
    pub cc: String,
    /// The `Bcc` field, comma-joined.
    pub bcc: String,
    /// The subject.
    pub subject: String,
    /// The body, as plain text.
    ///
    /// Plain text because that is what survives the round trip honestly: the queued draft's
    /// HTML was built by the composer that produced it, and re-opening it as HTML would ask
    /// this composer to adopt another's markup. A user editing an unsent message is editing
    /// their own words.
    pub body_text: String,
}
