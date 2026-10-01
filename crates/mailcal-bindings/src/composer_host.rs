//! The composer's requests to its host, over the FFI.
//!
//! The editor asks for something only the host can do well (a dialog over the window) by posting
//! a message on the one channel each client registers, [`COMPOSER_HOST_CHANNEL`]. A client hands
//! the message to [`parse_composer_host_request`], presents what it names, and runs the script the
//! matching `*_answer_script` builds. Parsing, the address rule and the answer's encoding are the
//! core's, so the four clients cannot disagree about what a request may carry
//! (`docs/composer-security.md`, Gate 2).

use mailcal_composer::{
    HostAnswer, HostRequestKind, LinkAnswer, answer_script, link_address, parse_host_request,
};

/// The name of the channel the editor posts its requests on: the script-message handler on Apple
/// and Linux, the injected object on Android, and the web message on Windows all use it.
pub const COMPOSER_HOST_CHANNEL: &str = "composerHost";

/// The name of the editor's request channel, for a client registering it.
#[must_use]
#[uniffi::export]
pub fn composer_host_channel() -> String {
    COMPOSER_HOST_CHANNEL.to_owned()
}

/// One request from the editor. Each carries the `id` its answer must be sent back with.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ComposerHostRequest {
    /// Make, change or remove the link on the selection: show the link dialog, starting from
    /// these values, and answer with [`composer_link_answer_script`].
    Link {
        /// The request's id.
        id: u64,
        /// The words the link is shown as: the selected text, or the existing link's.
        text: String,
        /// The existing link's target, or empty when the selection carries none.
        address: String,
        /// Whether the selection touches a link, so the dialog offers to remove it.
        removable: bool,
    },
}

/// Reads one message the editor posted, or `None` when it is not a request this product knows.
/// A client ignores a `None`: the editor treats a request nobody answers as cancelled.
#[must_use]
#[uniffi::export]
pub fn parse_composer_host_request(message: String) -> Option<ComposerHostRequest> {
    let request = parse_host_request(&message)?;
    Some(match request.kind {
        HostRequestKind::Link(link) => ComposerHostRequest::Link {
            id: request.id,
            text: link.text,
            address: link.address,
            removable: link.removable,
        },
    })
}

/// How the user closed the link dialog.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ComposerLinkAnswer {
    /// Link the selection, shown as `text`, to what the user typed as `address`.
    Apply {
        /// The words the link is shown as; empty keeps the selected words.
        text: String,
        /// The address as typed: completed and checked here, as [`composer_link_address`] does.
        address: String,
    },
    /// Take the link off the selection.
    Remove,
    /// Leave everything as it was.
    Cancel,
}

/// The address field's value as the link it would make, or `None` when it cannot be one. A dialog
/// enables its OK button only while this answers `Some`, and may show the completed address.
#[must_use]
#[uniffi::export]
pub fn composer_link_address(typed: String) -> Option<String> {
    link_address(&typed).map(|url| url.as_str().to_owned())
}

/// The script that delivers the link dialog's answer to request `id`. An `Apply` whose address
/// cannot be a link is sent as `Cancel`, so the editor never receives a target the core refused.
#[must_use]
#[uniffi::export]
pub fn composer_link_answer_script(id: u64, answer: ComposerLinkAnswer) -> String {
    let answer = match answer {
        ComposerLinkAnswer::Apply { text, address } => {
            link_address(&address).map_or(LinkAnswer::Cancel, |address| LinkAnswer::Apply {
                text,
                address,
            })
        }
        ComposerLinkAnswer::Remove => LinkAnswer::Remove,
        ComposerLinkAnswer::Cancel => LinkAnswer::Cancel,
    };
    answer_script(id, &HostAnswer::Link(answer))
}

/// The script that tells the editor which requests this host answers, so it asks for them rather
/// than drawing its own. Run it once the page has loaded; without it the editor draws everything
/// itself.
#[must_use]
#[uniffi::export]
pub fn composer_host_requests_script() -> String {
    "window.setComposerHostRequests([\"link\"]);".to_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        ComposerHostRequest, ComposerLinkAnswer, composer_link_address,
        composer_link_answer_script, parse_composer_host_request,
    };

    #[test]
    fn a_link_request_reaches_the_client_as_its_fields() {
        let message =
            r#"{"id":2,"request":{"link":{"text":"docs","address":"","removable":false}}}"#;
        assert_eq!(
            parse_composer_host_request(message.to_owned()),
            Some(ComposerHostRequest::Link {
                id: 2,
                text: "docs".to_owned(),
                address: String::new(),
                removable: false,
            })
        );
        assert_eq!(parse_composer_host_request("{\"id\":2}".to_owned()), None);
    }

    #[test]
    fn an_applied_address_is_completed_and_a_refused_one_cancels() {
        assert_eq!(
            composer_link_answer_script(
                1,
                ComposerLinkAnswer::Apply {
                    text: String::new(),
                    address: "example.com".to_owned()
                }
            ),
            r#"window.answerComposerRequest(1, {"link":{"apply":{"text":"","address":"https://example.com"}}});"#
        );
        assert_eq!(
            composer_link_answer_script(
                1,
                ComposerLinkAnswer::Apply {
                    text: String::new(),
                    address: "javascript:alert(1)".to_owned()
                }
            ),
            r#"window.answerComposerRequest(1, {"link":"cancel"});"#
        );
        assert_eq!(
            composer_link_address("someone@example.com".to_owned()).as_deref(),
            Some("mailto:someone@example.com")
        );
    }
}
