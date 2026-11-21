//! The signature body editor: the shared editor bundle hosted body-only inside Settings.
//!
//! It loads the same bundle through the same [`SecureWebView`] the composer uses, so the two hosts
//! carry one definition of the gates rather than two that can drift; authoring a signature *is*
//! authoring mail content. The one thing it does that the composer does not is insert a picture as
//! a self-contained `data:` URI ([`super::signature_pictures`]).
//!
//! Save sends the core a `SignatureIntent::Save` and leaves the rest to the page it reads back
//! (`docs/signatures.md`, "Changing the library"): Save is drawn from the page's own, held besides
//! while the core refuses the name typed, and what the save came to closes the editor or says why
//! it stays open.

use std::{cell::Cell, rc::Rc, sync::Arc};

use adw::prelude::*;
use gtk::{gio, glib};
use mailcal_bindings::{
    ControlState, Intent, MailcalApp, SignatureEntry, SignatureIntent, SignatureOutcome,
    SignatureResult, SignaturesPage,
};
use serde_json::json;
use webkit6::prelude::WebViewExt;

use super::{PageContext, page_box, signature_pictures};
use crate::{
    l10n,
    ui::{
        composer::EDITOR_HTML,
        composer_labels::editor_labels,
        controls, editor_paste,
        signature_image::refusal_text,
        webview::{DocumentKind, SecureWebView},
    },
};

const EDITOR: &str = "signature-editor";

/// An open editor, which follows every read of the page until it closes.
pub(super) struct Editor {
    app: Arc<MailcalApp>,
    /// The signature it saves: the entry's id, or the page's `new_id` as it was when it opened.
    id: String,
    /// The newest outcome acted on, starting from the one in force when it opened.
    seen: Cell<u64>,
    /// The page's Save as last read.
    save: Cell<ControlState>,
    /// The Save drawn, so it is drawn again only when it moves.
    drawn: Cell<Option<ControlState>>,
    name: gtk::Entry,
    add_image: gtk::Button,
    save_slot: gtk::Box,
    error: gtk::Label,
    web: webkit6::WebView,
    navigation: glib::WeakRef<gtk::Stack>,
    /// Tells the page the editor has gone.
    closed: Box<dyn Fn()>,
}

/// Opens the editor as a Settings detail on `entry`, or on a new signature when there is none.
pub(super) fn open(
    ctx: &PageContext,
    page: &SignaturesPage,
    entry: Option<&SignatureEntry>,
    closed: impl Fn() + 'static,
) -> Rc<Editor> {
    let (id, name, body_html, title) = match entry {
        Some(entry) => (
            entry.id.clone(),
            entry.name.clone(),
            // Fetched only now: the page carries names, so drawing the list never drags an
            // embedded logo across the FFI.
            ctx.app.signature_html(entry.id.clone()).unwrap_or_default(),
            entry.name.clone(),
        ),
        None => (
            page.new_id.clone(),
            l10n::settings_signatures_default_name().to_owned(),
            String::new(),
            l10n::settings_signatures_add().to_owned(),
        ),
    };
    if let Some(previous) = ctx.navigation.child_by_name(EDITOR) {
        ctx.navigation.remove(&previous);
    }
    let view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(&title, "")));
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    header.pack_start(&cancel);
    let save_slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    header.pack_end(&save_slot);
    let content = page_box(&title);

    let name_field = gtk::Entry::new();
    name_field.set_text(&name);
    name_field.set_placeholder_text(Some(l10n::settings_signatures_name_placeholder()));
    // The row it sits in labels itself from its title and cannot lend that to the entry, so the
    // entry carries its own name; otherwise a screen reader reaches an unnamed text field.
    name_field.update_property(&[gtk::accessible::Property::Label(
        l10n::settings_signatures_name_label(),
    )]);
    let name_row = adw::ActionRow::builder()
        .title(l10n::settings_signatures_name_label())
        .use_markup(false)
        .build();
    name_field.set_valign(gtk::Align::Center);
    name_field.set_hexpand(true);
    name_row.add_suffix(&name_field);
    let names = adw::PreferencesGroup::new();
    names.add(&name_row);
    content.append(&names);

    let body_label = gtk::Label::new(Some(l10n::settings_signatures_body_label()));
    body_label.set_xalign(0.0);
    content.append(&body_label);

    let web = SecureWebView::new(DocumentKind::Composer, ctx.sender.clone());
    super::super::composer_host::install(web.widget(), None, None);
    super::super::composer_scheme::follow(web.widget());
    // WebKit does not put its document on the accessibility bus here, so the host frame carries
    // the name; the same reason the composer labels the box around its editor rather than the
    // view itself. Named once the bundle has parsed, in `seed_body`.
    let frame = gtk::Frame::new(None);
    frame.set_accessible_role(gtk::AccessibleRole::Group);
    frame.set_vexpand(true);
    frame.set_child(Some(web.widget()));
    content.append(&frame);
    seed_body(&web, &body_html, &frame);

    let error = gtk::Label::new(None);
    error.add_css_class("error");
    error.set_xalign(0.0);
    error.set_wrap(true);
    error.set_visible(false);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let add_image = gtk::Button::with_label(l10n::settings_signatures_insert_image());
    signature_pictures::connect_image_picker(&add_image, web.widget(), &error, &ctx.window);
    actions.append(&add_image);
    // A pasted picture takes the same road as a picked one. The controller goes on the frame around
    // the view, never on the view itself, for the reason `editor_paste::install` states.
    let paste = signature_pictures::paste_answer(web.widget(), &error);
    editor_paste::install(&frame, web.widget(), paste.clone());
    web.set_paste_handler(paste);
    content.append(&actions);
    content.append(&error);

    view.add_top_bar(&header);
    view.set_content(Some(&content));
    ctx.navigation.add_named(&view, Some(EDITOR));
    ctx.navigation.set_visible_child_name(EDITOR);
    web.load(EDITOR_HTML, false);

    let editor = Rc::new(Editor {
        app: Arc::clone(&ctx.app),
        id,
        seen: Cell::new(page.outcome.as_ref().map_or(0, |outcome| outcome.serial)),
        save: Cell::new(page.save),
        drawn: Cell::new(None),
        name: name_field,
        add_image,
        save_slot,
        error,
        web: web.widget().clone(),
        navigation: ctx.navigation.downgrade(),
        closed: Box::new(closed),
    });
    let leaving = Rc::downgrade(&editor);
    cancel.connect_clicked(move |_| {
        if let Some(editor) = leaving.upgrade() {
            editor.close();
        }
    });
    let typed = Rc::downgrade(&editor);
    editor.name.connect_changed(move |_| {
        if let Some(editor) = typed.upgrade() {
            editor.draw_save();
        }
    });
    editor.draw_save();
    editor
}

impl Editor {
    /// Brings the editor to a read of the page: its Save, and what a save of this signature came
    /// to, acted on once.
    pub(super) fn follow(self: &Rc<Self>, page: &SignaturesPage) {
        self.save.set(page.save);
        self.draw_save();
        let Some(outcome) = outcome_for(page, self.seen.get(), &self.id) else {
            return;
        };
        self.seen.set(outcome.serial);
        match outcome.result {
            SignatureResult::Refused { reason } => {
                self.error.set_text(&refusal_text(reason));
                self.error.set_visible(true);
            }
            // Not written closes it too: the failed-save notice at the top of Settings says the
            // rest.
            SignatureResult::Saved | SignatureResult::Deleted | SignatureResult::NotWritten => {
                self.close();
            }
        }
    }

    /// Draws Save from the page's, held besides while the core refuses the name typed, and holds
    /// the fields with it.
    fn draw_save(self: &Rc<Self>) {
        let save = self.save.get();
        let control = save_control(save, &self.name.text());
        controls::hold_field(&self.name, save);
        controls::hold_field(&self.add_image, save);
        if self.drawn.replace(Some(control)) == Some(control) {
            return;
        }
        let editor = Rc::downgrade(self);
        let button = controls::button(
            l10n::settings_signatures_save(),
            control,
            false,
            move || {
                if let Some(editor) = editor.upgrade() {
                    editor.press_save();
                }
            },
        );
        button.add_css_class("suggested-action");
        while let Some(child) = self.save_slot.first_child() {
            self.save_slot.remove(&child);
        }
        self.save_slot.append(&button);
    }

    /// Sends the name as typed and the body as the bundle hands it back; the core trims the name.
    fn press_save(&self) {
        let app = Arc::clone(&self.app);
        let id = self.id.clone();
        let name = self.name.text().to_string();
        self.error.set_visible(false);
        self.web.evaluate_javascript(
            "window.signatureBody()",
            None,
            None,
            None::<&gio::Cancellable>,
            move |result| {
                let Some((body_html, body_plain)) =
                    result.ok().and_then(|value| parse_draft(&value.to_str()))
                else {
                    // The bundle has not parsed yet. Nothing is sent rather than an empty body
                    // over a signature the person was editing.
                    log::warn!("the signature editor is not ready to be saved yet");
                    return;
                };
                app.dispatch(Intent::Signature {
                    intent: SignatureIntent::Save {
                        id,
                        name,
                        body_html,
                        body_plain,
                    },
                });
            },
        );
    }

    /// Puts the Settings page back in place of the editor.
    ///
    /// On the next turn of the loop: removing this detail destroys the WebView, and the call may
    /// come from inside one of its own callbacks.
    fn close(&self) {
        (self.closed)();
        let navigation = self.navigation.clone();
        glib::idle_add_local_once(move || {
            let Some(navigation) = navigation.upgrade() else {
                return;
            };
            navigation.set_visible_child_name("settings");
            if let Some(editor) = navigation.child_by_name(EDITOR) {
                navigation.remove(&editor);
            }
        });
    }
}

/// The outcome on `page` an editor of signature `id` acts on, having acted on those up to
/// `seen`: a newer one, for its own signature.
fn outcome_for<'a>(page: &'a SignaturesPage, seen: u64, id: &str) -> Option<&'a SignatureOutcome> {
    page.outcome
        .as_ref()
        .filter(|outcome| outcome.serial > seen && outcome.id == id)
}

/// Save as drawn: the page's, held besides while the core refuses `name`.
fn save_control(save: ControlState, name: &str) -> ControlState {
    ControlState {
        enabled: save.enabled
            && mailcal_bindings::signature_name_refusal(name.to_owned()).is_none(),
        running: save.running,
    }
}

/// Loads the signature into the bundle once it has parsed.
///
/// The labels go first and the body second: `setSignatureBody` carries **this** surface's
/// placeholder, and the bundle's default ("Write your message") is the composer's wording, which
/// is a lie here. A new signature is seeded too, empty body and all, because that call is what
/// carries the placeholder.
fn seed_body(web: &SecureWebView, body_html: &str, host: &gtk::Frame) {
    let labels = editor_labels();
    let body = json!(body_html).to_string();
    let placeholder = json!(l10n::settings_signatures_placeholder()).to_string();
    // Weak: the frame holds the view this closure is attached to.
    let host = host.downgrade();
    web.connect_finished(move |view| {
        let Some(host) = host.upgrade() else {
            return;
        };
        host.update_property(&[gtk::accessible::Property::Label(
            l10n::settings_signatures_placeholder(),
        )]);
        // Writing the signature is the only thing this screen is for, so the caret opens in it.
        // Asked for rather than assumed: the shared bundle focuses nothing of its own accord,
        // because in the composer the caret belongs in To (docs/contacts.md §4).
        let announce = super::super::composer_host::announce_script();
        let scheme = super::super::composer_scheme::current_script();
        let script = format!(
            "window.setComposerLabels({labels});{scheme}{announce}\
             window.setSignatureBody({body}, {placeholder});window.focusComposerBody();"
        );
        view.evaluate_javascript(&script, None, None, None::<&gio::Cancellable>, |_| {});
    });
}

/// The HTML to store and its plain-text rendering, as `window.signatureBody()` hands them back.
fn parse_draft(json: &str) -> Option<(String, String)> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let html = value.get("body_html")?.as_str()?.to_owned();
    let plain = value
        .get("body_plain")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Some((html, plain))
}

#[cfg(test)]
#[path = "signature_editor_tests.rs"]
mod tests;
