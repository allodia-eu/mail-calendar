//! Native GTK composer chrome around a fresh, hardened WebKit editor per draft.

use std::{
    cell::{Cell, RefCell},
    fmt::Write as _,
    rc::Rc,
    sync::Arc,
};

use adw::prelude::*;
use gtk::{AccessibleRole, accessible::Property as AccessibleProperty, gio};
use mailcal_bindings::MailcalApp;
use serde_json::json;
use webkit6::prelude::WebViewExt;

use super::{
    AppInput,
    composer_attach::{
        PictureAnswer, accept_files, connect_file_picker, install_drop_target, render_files,
        show_in_message,
    },
    composer_draft::{DraftGuard, HeaderValues},
    composer_drafts::Autosave,
    composer_fields::{ComposerFields, connect_save_draft, connect_send, read_document},
    composer_header::{RecipientRows, add_from_row, entry_row, from_picker, recipient_rows},
    composer_model::{ComposeContext, ComposeKind, plain_text_seed_script},
    composer_signature::SignatureControl,
    editor_paste,
    reader::ComposerHost,
    recipients::{self, RecipientField},
    webview::{DocumentKind, SecureWebView},
};
use crate::l10n;

/// The shared editor bundle, hosted by the composer here and by the Settings signature editor
/// body-only; one file, so the two hosts cannot drift.
pub(super) const EDITOR_HTML: &str = include_str!("../../../composer/dist/editor.html");

pub(crate) struct ComposerPane {
    root: gtk::Box,
    active_generation: Cell<Option<u64>>,
    error: RefCell<Option<gtk::Label>>,
    send: RefCell<Option<gtk::Button>>,
    /// The three recipient fields, kept so their suggestion popovers can be dropped before the
    /// widget tree they hang off goes.
    fields: RefCell<Vec<Rc<RecipientField>>>,
    /// The draft's signature control. The pane owns the only strong reference; the editor, the
    /// From picker and the menu action all reach it weakly; so tearing the pane down frees it.
    signature: RefCell<Option<Rc<SignatureControl>>>,
    /// The open draft's unsaved-work guard, and the generation it has already been asked about,
    /// so a re-render cannot ask twice for one navigation.
    draft: RefCell<Option<DraftGuard>>,
    checked_generation: Cell<Option<u64>>,
    /// The open draft's autosave: the one timer that notices a change and stores the message once
    /// the composer has gone quiet (`docs/drafts.md`).
    autosave: Autosave,
    /// The line the draft's last save is reported on. Its own label rather than the error line's,
    /// so a save that failed cannot erase a send that did.
    hint: RefCell<Option<gtk::Label>>,
}

impl ComposerPane {
    pub(crate) fn new() -> Self {
        Self {
            root: gtk::Box::new(gtk::Orientation::Vertical, 0),
            active_generation: Cell::new(None),
            error: RefCell::new(None),
            send: RefCell::new(None),
            fields: RefCell::new(Vec::new()),
            signature: RefCell::new(None),
            draft: RefCell::new(None),
            checked_generation: Cell::new(None),
            autosave: Autosave::default(),
            hint: RefCell::new(None),
        }
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// Builds the chrome and the editor for one draft.
    ///
    /// `window` is whichever window hosts it, because the file chooser and the drop target open
    /// over their own window: the mailbox for the pane's composer, the composer window for a draft
    /// raised inside a reading window (`docs/reading-window.md`).
    pub(crate) fn show(
        &self,
        generation: u64,
        request: &ComposeContext,
        accounts: &[(String, String)],
        app: Option<&Arc<MailcalApp>>,
        window: &impl IsA<gtk::Window>,
        sender: relm4::Sender<AppInput>,
    ) {
        if self.active_generation.get() == Some(generation) {
            return;
        }
        self.teardown();
        self.active_generation.set(Some(generation));

        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        // In a window of its own this bar *is* the titlebar, so it keeps the desktop's controls
        // wherever that desktop puts them; in the pane, the surface above it carries the mailbox
        // window's own. The bar still says what the draft is; the window is named after its
        // subject, which is what the OS window list reads (`docs/reading-window.md`).
        header.set_show_start_title_buttons(matches!(request.host, ComposerHost::Window(_)));
        header.set_title_widget(Some(&adw::WindowTitle::new(
            compose_title(request.kind),
            "",
        )));
        let cancel = gtk::Button::with_label(l10n::action_cancel());
        let input_sender = sender.clone();
        let host = request.host;
        cancel.connect_clicked(move |_| input_sender.emit(AppInput::CancelComposer(host)));
        header.pack_start(&cancel);
        let send_button = gtk::Button::with_label(l10n::action_send());
        send_button.add_css_class("suggested-action");
        header.pack_end(&send_button);
        toolbar.add_top_bar(&header);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        let form = gtk::Grid::new();
        form.set_column_spacing(10);
        form.set_row_spacing(8);
        let from = from_picker(accounts, request.initial_from.as_deref());
        add_from_row(&form, 0, &from);
        let RecipientRows { to, cc, bcc } = recipient_rows(&form, request, app);
        to.focus_on_activate();
        // The caret opens where the work starts. The body's half of this is in `seed_editor`, and
        // has to wait for the bundle; To can take it now.
        if !request.opens_in_body() {
            to.focus_entry();
        }
        self.fields
            .replace(vec![Rc::clone(&to), Rc::clone(&cc), Rc::clone(&bcc)]);
        // Editable whatever the composer is for. A reply and a forward open with the derived
        // `Re:`/`Fwd:` already in the field, and renaming a thread there is what the user means by
        // editing it: the core takes the field's value over its own derivation.
        let subject = entry_row(&form, 4, l10n::compose_subject(), &request.subject, false);
        content.append(&form);

        // The composer's action bar; things you do *to* the message, as against the From/To/Cc
        // fields you address it with. The signature picker joins it below, once the editor it
        // writes into exists (docs/signatures.md).
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::Start);
        let attach = gtk::Button::with_label(l10n::action_attach());
        actions.append(&attach);
        // Never disabled: a draft is unfinished by definition, so there is no state a composer
        // can be in that this refuses, and pressing it on an unchanged message reaches no server
        // (`docs/drafts.md`).
        let save_draft = gtk::Button::with_label(l10n::action_save_draft());
        actions.append(&save_draft);
        content.append(&actions);
        let file_list = gtk::ListBox::new();
        file_list.add_css_class("boxed-list");
        content.append(&file_list);
        // A share opens the composer already holding its files; every other route starts
        // empty and fills this from the picker or a drop below (docs/os-integration.md).
        let files = Rc::new(RefCell::new(request.files.clone()));
        render_files(&file_list, &files);
        connect_file_picker(&attach, &file_list, &files, window);

        let error = gtk::Label::new(Some(l10n::compose_prepare_error()));
        error.add_css_class("error");
        error.set_visible(false);
        content.append(&error);
        // How the draft's last save ended. A hint and never a gate: no state here stops the
        // composer being closed, and none of it is worth a dialog.
        let hint = gtk::Label::new(None);
        hint.add_css_class("dim-label");
        hint.set_visible(false);
        content.append(&hint);

        let web = SecureWebView::new(DocumentKind::Composer, sender.clone());
        // One route into the composer for files, taken twice, so a drop and a paste differ in the
        // one place they should: a drop asks what a picture is for, a paste has already been told.
        let take_files = |answer| {
            accept_files(
                answer,
                show_in_message(web.widget()),
                &file_list,
                &files,
                window,
                &error,
            )
        };
        install_drop_target(&content, take_files(PictureAnswer::Ask));
        // A pasted picture takes the same road as a dropped one, because this toolkit's WebView
        // hands the page no clipboard files at all (`editor_paste`). The context menu's own item
        // is one route into it; the chord is the other, and is wired below, once the box that
        // holds the editor exists to carry the controller.
        let paste = editor_paste::PasteAnswer {
            picture: editor_paste::paste_into_message(web.widget(), &error),
            files: take_files(PictureAnswer::ShowInMessage),
        };
        web.set_paste_handler(paste.clone());
        web.widget()
            .update_property(&[AccessibleProperty::Label(l10n::compose_body())]);
        let signature = request
            .seeds_signature
            .then(|| SignatureControl::new(app, request.kind, accounts, &from, web.widget()))
            .flatten();
        if let Some(control) = &signature {
            actions.append(control.widget());
        }
        let editor_host = gtk::Box::new(gtk::Orientation::Vertical, 0);
        editor_host.set_accessible_role(AccessibleRole::Group);
        editor_host.set_hexpand(true);
        editor_host.set_vexpand(true);
        editor_host.append(web.widget());
        editor_paste::install(&editor_host, web.widget(), paste);
        // Captured once the seeding script returns, so the guard measures the body against what
        // the quote and signature put there rather than against empty.
        let seed = Rc::new(RefCell::new(None));
        seed_editor(
            &web,
            request,
            &editor_host,
            signature.as_ref(),
            Rc::clone(&seed),
        );
        content.append(&editor_host);
        toolbar.set_content(Some(&content));
        self.root.append(&toolbar);

        send_button.set_sensitive(!recipients::is_empty(&to.text()));
        let sensitive_button = send_button.clone();
        to.connect_changed(move |field| {
            sensitive_button.set_sensitive(!recipients::is_empty(field));
        });
        self.draft.replace(Some(DraftGuard::new(
            web.widget().clone(),
            RecipientRows {
                to: Rc::clone(&to),
                cc: Rc::clone(&cc),
                bcc: Rc::clone(&bcc),
            },
            subject.clone(),
            Rc::clone(&files),
            HeaderValues {
                to: request.initial_to.clone(),
                cc: request.initial_cc.clone(),
                bcc: request.initial_bcc.clone(),
                subject: request.subject.clone(),
            },
            // A forward's staged files are not work to lose: they are still in the mailbox, so
            // they are the baseline rather than a draft. A share's are the user's own choice.
            if request.kind == ComposeKind::Forward {
                request.files.len()
            } else {
                0
            },
            seed,
        )));
        let fields = ComposerFields {
            request: request.clone(),
            accounts: accounts.to_vec(),
            from,
            to,
            cc,
            bcc,
            subject,
            files,
        };
        connect_send(
            &send_button,
            web.widget(),
            &fields,
            error.clone(),
            sender.clone(),
        );
        connect_save_draft(&save_draft, web.widget(), &fields, sender.clone());
        // The idle save takes the same route the button does, so the two cannot come to store
        // different messages; the timer samples both the editor and the header fields, because
        // the page has no channel back to this host and a recipient field keeps one change
        // callback, which Send already owns.
        let idle_editor = web.widget().clone();
        let idle_fields = fields.clone();
        self.autosave.start(web.widget(), fields, move || {
            read_document(
                &idle_editor,
                &idle_fields,
                &sender,
                AppInput::SaveComposerDraft,
                || {},
            );
        });
        web.load(EDITOR_HTML, false);
        self.hint.replace(Some(hint));
        self.error.replace(Some(error));
        self.send.replace(Some(send_button));
        self.signature.replace(signature);
    }

    /// Draws what the draft's last save said, or takes the line away when there is nothing to
    /// say. A hint and never a gate: no state here stops the composer being closed.
    pub(crate) fn show_draft_hint(&self, model: &super::AppModel, host: ComposerHost) {
        let Some(label) = self.hint.borrow().clone() else {
            return;
        };
        let status = model.draft_status_of(host);
        match super::composer_drafts::draft_hint_text(status) {
            Some(text) => {
                label.set_text(text);
                if super::composer_drafts::draft_hint_failed(status) {
                    label.remove_css_class("dim-label");
                    label.add_css_class("error");
                } else {
                    label.remove_css_class("error");
                    label.add_css_class("dim-label");
                }
                label.set_visible(true);
            }
            None => label.set_visible(false),
        }
    }

    pub(crate) fn show_error(&self, text: &str) {
        if let Some(error) = self.error.borrow().as_ref() {
            error.set_text(text);
            error.set_visible(true);
        }
        if let Some(send) = self.send.borrow().as_ref() {
            send.set_sensitive(true);
        }
    }

    pub(crate) fn is_active(&self, generation: u64) -> bool {
        self.active_generation.get() == Some(generation)
    }

    /// Asks the open draft whether anything would be lost, once per navigation.
    ///
    /// The generation guard is what makes it once: `render` runs on every update, and the model
    /// cannot clear the request itself because it renders behind a shared reference.
    pub(crate) fn check_draft(&self, generation: u64, sender: &relm4::Sender<AppInput>) {
        if self.checked_generation.get() == Some(generation) {
            return;
        }
        self.checked_generation.set(Some(generation));
        match self.draft.borrow().as_ref() {
            Some(draft) => draft.check(sender),
            // No draft to lose; the pane is torn down or was never shown.
            None => sender.emit(AppInput::ComposerDraftChecked(false)),
        }
    }

    pub(crate) fn teardown(&self) {
        // Before the widgets go: a tick left running would read an editor that has been dropped.
        self.autosave.stop();
        self.hint.replace(None);
        // Dropped, not just detached: each field's suggestion popover lives in its own surface
        // and unparents itself on drop.
        self.fields.take();
        self.signature.take();
        self.draft.take();
        self.checked_generation.set(None);
        while let Some(child) = self.root.first_child() {
            self.root.remove(&child);
        }
        self.active_generation.set(None);
        self.error.replace(None);
        self.send.replace(None);
    }
}

/// Every string the shared editor's own chrome draws, in the bundle's key names.
///
/// One map for the composer **and** the Settings signature editor: the bundle keeps no
/// translations of its own, and a host that sends a partial map leaves those controls in English
/// with nothing to say so (`cargo xtask check-composer-labels`).
pub(super) fn editor_labels() -> serde_json::Value {
    json!({
        "placeholder": l10n::editor_placeholder(),
        "bold": l10n::editor_bold(),
        "italic": l10n::editor_italic(),
        "underline": l10n::editor_underline(),
        "fontSize": l10n::editor_font_size(),
        "sizeNormal": l10n::editor_size_normal(),
        "sizeSmall": l10n::editor_size_small(),
        "sizeLarge": l10n::editor_size_large(),
        "sizeHuge": l10n::editor_size_huge(),
        "bulletedList": l10n::editor_bulleted_list(),
        "numberedList": l10n::editor_numbered_list(),
        "indent": l10n::editor_indent(),
        "outdent": l10n::editor_outdent(),
        "textColour": l10n::editor_text_colour(),
        "colourAutomatic": l10n::editor_colour_automatic(),
        "highlight": l10n::editor_highlight(),
        "highlightNone": l10n::editor_highlight_none(),
        "table": l10n::editor_table(),
        "insertTable": l10n::editor_insert_table(),
        "insertRowAbove": l10n::editor_insert_row_above(),
        "insertRowBelow": l10n::editor_insert_row_below(),
        "insertColumnLeft": l10n::editor_insert_column_left(),
        "insertColumnRight": l10n::editor_insert_column_right(),
        "deleteRow": l10n::editor_delete_row(),
        "deleteColumn": l10n::editor_delete_column(),
        "deleteTable": l10n::editor_delete_table(),
    })
}

/// Sends the editor everything an open draft needs, once the bundle has parsed.
///
/// This is an **ordering** contract, and one that fails silently. A `window.*` hook called before
/// the bundle parses lands on an undefined function and `evaluate_javascript` reports nothing, so
/// every open-time hook belongs in this one batch. Within it the quote goes first and the
/// signature second: the seam places the region above `.allodia-quote` when there is one, which is
/// what makes a reply read message → signature → original.
fn seed_editor(
    web: &SecureWebView,
    request: &ComposeContext,
    editor_host: &gtk::Box,
    signature: Option<&Rc<SignatureControl>>,
    seed: Rc<RefCell<Option<String>>>,
) {
    let labels = editor_labels();
    let quote = request.quote.clone();
    let body = plain_text_seed_script(request.initial_body.as_deref());
    // Read here, not in the closure: the request is borrowed, the closure outlives this call.
    let opens_in_body = request.opens_in_body();
    let editor_host = editor_host.clone();
    // Weak: the control owns the view this closure is attached to.
    let signature = signature.map(Rc::downgrade);
    web.connect_finished(move |view| {
        editor_host.update_property(&[AccessibleProperty::Label(l10n::editor_placeholder())]);
        let mut script = format!("window.setComposerLabels({labels});");
        if let Some(quote) = &quote
            && let Ok(encoded) = serde_json::to_string(quote)
        {
            let _ = write!(script, "window.setComposerQuote({encoded});");
        } else if let Some(body) = &body {
            script.push_str(body);
        }
        if let Some(control) = signature.as_ref().and_then(std::rc::Weak::upgrade) {
            script.push_str(&control.seed_script());
        }
        if opens_in_body {
            script.push_str("window.focusComposerBody();");
        }
        let seed = Rc::clone(&seed);
        let seeded_view = view.clone();
        view.evaluate_javascript(&script, None, None, None::<&gio::Cancellable>, move |_| {
            let seed = Rc::clone(&seed);
            seeded_view.evaluate_javascript(
                "composerDocument()",
                None,
                None,
                None::<&gio::Cancellable>,
                move |result| {
                    if let Ok(value) = result {
                        seed.replace(Some(value.to_str().to_string()));
                    }
                },
            );
        });
    });
}

fn compose_title(kind: ComposeKind) -> &'static str {
    match kind {
        ComposeKind::New => l10n::compose_title_new(),
        ComposeKind::Reply => l10n::action_reply(),
        ComposeKind::ReplyAll => l10n::action_reply_all(),
        ComposeKind::Forward => l10n::action_forward(),
    }
}

#[cfg(test)]
mod tests {
    use super::EDITOR_HTML;

    #[test]
    fn editor_has_an_accessible_name_before_host_localization_runs() {
        assert!(EDITOR_HTML.contains("aria-label=\"Write your message\""));
    }
}
