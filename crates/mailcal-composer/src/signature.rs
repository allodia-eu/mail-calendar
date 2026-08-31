//! The sender's signature block embedded in an outgoing message.
//!
//! A signature is a **standalone, reusable** piece of authored content (see
//! `docs/signatures.md`): the user writes it once in Settings, assigns it per account for new
//! messages and for replies/forwards, and the composer seeds it into the body where it stays
//! editable before send. It rides the document as one [`Signature`] block: the same shape as
//! [`Quote`](crate::Quote), and for the same reason: it is a raw HTML fragment the composer emits
//! **verbatim** rather than something it builds from nodes.
//!
//! Security: `body_html` is authored by the user and then round-tripped through the host's
//! WebView editor, so it is untrusted on the way back. The core sanitises it on submit: the same
//! gate a quoted original passes (`docs/composer-security.md`, Gate 10), and rewrites its inline
//! `data:` images into `cid:` parts, so a logo renders in readers that block `data:`.

use core::fmt;

use serde::{Deserialize, Serialize};

/// The sender's signature, as it rides the compose document.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    /// The signature's HTML fragment, with any images inline as `data:` URIs. The composer
    /// emits it verbatim; the core sanitises it on submit and rewrites the images to `cid:`.
    pub body_html: String,
    /// The signature's plain-text rendering, for the outgoing message's `text/plain` part.
    /// Empty when the signature is images-only.
    #[serde(default)]
    pub body_plain: String,
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Signature")
            .field("body_html_len", &self.body_html.len())
            .field("body_plain_len", &self.body_plain.len())
            .finish()
    }
}
