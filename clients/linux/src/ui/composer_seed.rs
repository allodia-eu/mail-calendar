//! What an open draft's editor is sent once its bundle has parsed: the labels, the scheme, the
//! channel, the body, the signature and the quote's attribution, in that order.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use gtk::{accessible::Property as AccessibleProperty, gio};
use webkit6::prelude::WebViewExt;

use super::{
    composer_labels::editor_labels,
    composer_model::{ComposeContext, attribution_script, body_seed_script},
    composer_quote_style::QuoteStylePicker,
    composer_signature::SignatureControl,
    webview::SecureWebView,
};
use crate::l10n;

/// Sends the editor everything an open draft needs, once the bundle has parsed.
///
/// This is an **ordering** contract, and one that fails silently. A `window.*` hook called before
/// the bundle parses lands on an undefined function and `evaluate_javascript` reports nothing, so
/// every open-time hook belongs in this one batch. Within it the body goes first, since each seed
/// assigns the whole body, and the signature second: the seam places the region above
/// `.allodia-quote` when there is one, which is what makes a reply read message → signature →
/// original. A reopened quote's attribution is completed last, once the body holding it is in.
pub(super) fn seed_editor(
    web: &SecureWebView,
    request: &ComposeContext,
    editor_host: &gtk::Box,
    signature: Option<&Rc<SignatureControl>>,
    quote_style: Option<QuoteStylePicker>,
    seed: Rc<RefCell<Option<String>>>,
) {
    let labels = editor_labels();
    let body = body_seed_script(request);
    let attribution = attribution_script(request);
    // Read here, not in the closure: the request is borrowed, the closure outlives this call.
    let opens_in_body = request.opens_in_body();
    let moved_signature = request.moved.as_ref().map(|moved| moved.signature.clone());
    let editor_host = editor_host.clone();
    // Weak: the control owns the view this closure is attached to.
    let signature = signature.map(Rc::downgrade);
    web.connect_finished(move |view| {
        editor_host.update_property(&[AccessibleProperty::Label(l10n::editor_placeholder())]);
        let mut script = format!("window.setComposerLabels({labels});");
        script.push_str(&super::composer_scheme::current_script());
        // Before the body is seeded and before the channel is announced, so the first caret and
        // scroll it reports are already the seeded message's (`docs/composer-layout.md`).
        script.push_str("window.useFramedComposer();");
        script.push_str(&super::composer_host::announce_script());
        if let Some(body) = &body {
            script.push_str(body);
        }
        // Only a composer that manages a signature has a control; one that does not never touches
        // the region its body already carries, and a moved one carries on without seeding it.
        if let Some(control) = signature.as_ref().and_then(std::rc::Weak::upgrade) {
            match &moved_signature {
                Some(moved) => control.carry_on(moved),
                None => script.push_str(&control.seed_script()),
            }
        }
        if let Some(attribution) = &attribution {
            script.push_str(attribution);
        }
        if opens_in_body {
            script.push_str("window.focusComposerBody();");
        }
        let seed = Rc::clone(&seed);
        let quote_style = quote_style.clone();
        let seeded_view = view.clone();
        let ready = editor_host.clone();
        view.evaluate_javascript(&script, None, None, None::<&gio::Cancellable>, move |_| {
            let seed = Rc::clone(&seed);
            let ready = ready.clone();
            seeded_view.evaluate_javascript(
                "composerDocument()",
                None,
                None,
                None::<&gio::Cancellable>,
                move |result| {
                    if let Ok(value) = result {
                        let document = value.to_str().to_string();
                        // The picker opens on the style the quote is in, which for a reopened
                        // message is the one it was saved in, not the app default.
                        if let Some(picker) = &quote_style {
                            picker.reflect(&document);
                        }
                        seed.replace(Some(document));
                        // Named for a test or script (`docs/test-agent.md`) only now: from here a
                        // composer nobody has written in closes without asking.
                        ready.set_widget_name("composer-editor");
                    }
                },
            );
        });
    });
}
