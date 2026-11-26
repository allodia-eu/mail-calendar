//! The page the composer's message is written on, over the FFI (`docs/reading-dark-mode.md`,
//! "Writing in a dark app").
//!
//! A client decides the page's [`MessageScheme`] exactly as it does a reading view's, and puts the
//! editor on it with [`composer_page_script`]. What the editor then asks about the colours in its
//! message is answered by the core ([`composer_colours_answer_script`]), and the page its toolbar's
//! sun or moon asks for is the core's to store ([`MailcalApp::composer_background_choice`]).
//! Neither reaches a client as a [`crate::ComposerHostRequest`], so a client hands every message
//! the editor posts to both, as it does to `composer_reply_draft_report`.

use mailcal_composer::{
    HostAnswer, HostRequestKind, PageScheme, answer_script, page_colours, page_preload_script,
    page_script, parse_host_request,
};

use crate::{MailcalApp, MessageScheme};

impl From<MessageScheme> for PageScheme {
    fn from(scheme: MessageScheme) -> Self {
        match scheme {
            MessageScheme::Light => Self::Light,
            MessageScheme::Dark => Self::Dark,
        }
    }
}

/// The script that puts the editor's message on `scheme`'s page: the reading view's canvas, and its
/// colours moved as the reading view moves them. Run it on load and whenever the scheme changes. A
/// host that never runs it keeps the editor's message on a light page that moves nothing, and is
/// never asked about colours.
#[must_use]
#[uniffi::export]
pub fn composer_page_script(scheme: MessageScheme) -> String {
    page_script(scheme.into())
}

/// The same page as a script the web view runs before the editor's document does (a user script
/// injected at the start of the document), so the editor's first frame is already drawn on it
/// rather than on the light page it starts on. [`composer_page_script`] still names the page once
/// the document has loaded, and on every change after.
#[must_use]
#[uniffi::export]
pub fn composer_page_preload_script(scheme: MessageScheme) -> String {
    page_preload_script(scheme.into())
}

/// The script that answers a colours request the editor posted, or `None` when `message` is not
/// one. The answer is the core's alone, so a client runs it as it is.
#[must_use]
#[uniffi::export]
pub fn composer_colours_answer_script(message: String) -> Option<String> {
    let request = parse_host_request(&message)?;
    let HostRequestKind::Colours(colours) = request.kind else {
        return None;
    };
    Some(answer_script(
        request.id,
        &HostAnswer::Colours(page_colours(&colours)),
    ))
}

#[uniffi::export]
impl MailcalApp {
    /// Hands the core a message the editor posted, and answers whether it was the toolbar's sun or
    /// moon. When it was, the reader's one dark-messages preference is set to the page it asked
    /// for, exactly as the reading view's toggle sets it, and every open message and composer
    /// follows (`docs/reading-dark-mode.md`, rule 2).
    pub fn composer_background_choice(&self, message: String) -> bool {
        let Some(request) = parse_host_request(&message) else {
            return false;
        };
        let HostRequestKind::Background(choice) = request.kind else {
            return false;
        };
        self.dispatch_preference(crate::PreferenceIntent::SetDarkMessageBackground {
            dark: choice.scheme == PageScheme::Dark,
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{composer_colours_answer_script, composer_page_script};
    use crate::{MessageScheme, message_canvas};

    #[test]
    fn the_page_is_the_reading_views_canvas() {
        for scheme in [MessageScheme::Light, MessageScheme::Dark] {
            let canvas = message_canvas(scheme);
            let script = composer_page_script(scheme);
            assert!(
                script.contains(&format!("\"background\":\"{}\"", canvas.background)),
                "{script}"
            );
            assert!(
                script.contains(&format!("\"text\":\"{}\"", canvas.foreground)),
                "{script}"
            );
        }
    }

    #[test]
    fn only_a_colours_request_is_answered() {
        let colours =
            r##"{"id":3,"request":{"colours":{"scheme":"dark","pieces":[{"fill":"#ffffff"}]}}}"##;
        assert_eq!(
            composer_colours_answer_script(colours.to_owned()).as_deref(),
            Some(r##"window.answerComposerRequest(3, {"colours":["#1e1e1e"]});"##)
        );
        let link = r#"{"id":3,"request":{"link":{"text":"","address":"","removable":false}}}"#;
        assert_eq!(composer_colours_answer_script(link.to_owned()), None);
    }
}
