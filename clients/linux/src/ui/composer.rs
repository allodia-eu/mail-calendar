//! Native GTK composer chrome around a fresh, hardened WebKit editor per draft.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use adw::prelude::*;
use gtk::{AccessibleRole, accessible::Property as AccessibleProperty};
use mailcal_bindings::MailcalApp;

use super::{
    AppInput,
    composer_attach::{
        PictureAnswer, accept_files, connect_file_picker, install_drop_target, show_in_message,
    },
    composer_draft::{DraftGuard, HeaderValues},
    composer_drafts::Autosave,
    composer_fields::{ComposerFields, connect_save_draft, connect_send, read_document},
    composer_file_tiles::{ComposerFileStore, FileTiles},
    composer_header::{RecipientRows, add_from_row, entry_row, from_picker, recipient_rows},
    composer_importance::{ImportanceControl, ImportanceStore},
    composer_layout::ComposerColumn,
    composer_model::{ComposeContext, compose_title},
    composer_pop_out::{connect_pop_out, pop_out_button},
    composer_quote_style::QuoteStylePicker,
    composer_reply_draft::{ReplyDraft, ReplyDraftCore},
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
    /// The open draft's guard, which leaves it and answers Discard, and the generation it has
    /// already been asked about, so a re-render cannot leave twice for one navigation.
    ///
    /// Shared, because a composer window's close handler is connected before any draft is shown
    /// and has to reach whichever draft is open when the window closes.
    draft: Rc<RefCell<Option<Rc<DraftGuard>>>>,
    checked_generation: Cell<Option<u64>>,
    /// The open draft's autosave: the one timer that notices a change and stores the message once
    /// the composer has gone quiet (`docs/drafts.md`).
    autosave: Autosave,
    /// The line the draft's last save is reported on. Its own label rather than the error line's,
    /// so a save that failed cannot erase a send that did.
    hint: RefCell<Option<gtk::Label>>,
    /// The open draft's one scroll, which sizes the editor and follows its caret. The editor's
    /// channel reaches it weakly, so this is what keeps it.
    column: RefCell<Option<Rc<ComposerColumn>>>,
    /// Draft a reply, for a reply while the assistant is offered (`docs/ai.md`).
    reply_draft: RefCell<Option<Rc<ReplyDraft>>>,
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
            draft: Rc::new(RefCell::new(None)),
            checked_generation: Cell::new(None),
            autosave: Autosave::default(),
            hint: RefCell::new(None),
            column: RefCell::new(None),
            reply_draft: RefCell::new(None),
        }
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// The cell holding the open draft's guard, for a composer window's close handler: closing
    /// the window is leaving the draft (`docs/drafts.md`).
    pub(crate) fn draft_cell(&self) -> Rc<RefCell<Option<Rc<DraftGuard>>>> {
        Rc::clone(&self.draft)
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
        // wherever that desktop puts them; in the pane, the header bar spanning the mailbox window
        // carries its controls, at both ends. The bar still says what the draft is; the window is
        // named after its subject, which is what the OS window list reads
        // (`docs/reading-window.md`).
        let own_window = matches!(request.host, ComposerHost::Window(_));
        header.set_show_start_title_buttons(own_window);
        header.set_show_end_title_buttons(own_window);
        header.set_title_widget(Some(&adw::WindowTitle::new(
            compose_title(request.kind),
            "",
        )));
        // Where Cancel would be: on a desktop, leaving is a click elsewhere or the window's own
        // close, so this is the one way to throw the draft away (`docs/drafts.md`). Wired below,
        // once the guard that answers it exists.
        let discard = gtk::Button::with_label(l10n::action_discard());
        header.pack_start(&discard);
        // Packed first, so it is the bar's last item, past Send.
        let pop_out = pop_out_button(request);
        if let Some(button) = &pop_out {
            header.pack_end(button);
        }
        let send_button = gtk::Button::with_label(l10n::action_send());
        send_button.add_css_class("suggested-action");
        header.pack_end(&send_button);
        toolbar.add_top_bar(&header);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_top(12);
        // Nothing under the editor: its box ends where the column's visible part does
        // (`docs/composer-layout.md`, rule 2).
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
        // Wrapping, so a large text size or a narrow pane moves a button to the next line rather
        // than pushing the column wider than the pane.
        let actions = adw::WrapBox::new();
        actions.set_child_spacing(8);
        actions.set_line_spacing(8);
        let attach = gtk::Button::with_label(l10n::action_attach());
        actions.append(&attach);
        // Never disabled: a draft is unfinished by definition, so there is no state a composer
        // can be in that this refuses, and pressing it on an unchanged message reaches no server
        // (`docs/drafts.md`).
        let save_draft = gtk::Button::with_label(l10n::action_save_draft());
        actions.append(&save_draft);
        content.append(&actions);

        // The notes, under the action row (`docs/composer-layout.md`, rule 1).
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
        // Draft a reply's own notes, filled once the editor it writes into exists.
        let draft_notes = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&draft_notes);

        // The composition's files, as the core holds them: a forward's originals, a stored
        // draft's own, a share's; the picker, a drop and a paste add to them.
        let store = app.map(|app| Arc::clone(app) as Arc<dyn ComposerFileStore>);
        let files = FileTiles::new(store, &request.composition);
        content.append(files.widget());
        connect_file_picker(&attach, &files, window);

        let web = SecureWebView::new(DocumentKind::Composer, sender.clone());
        super::composer_scheme::follow(web.widget());
        // One route into the composer for files, taken twice, so a drop and a paste differ in the
        // one place they should: a drop asks what a picture is for, a paste has already been told.
        let take_files = |answer| {
            accept_files(
                answer,
                show_in_message(web.widget()),
                &files,
                window,
                &error,
            )
        };
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
            .manages_signature()
            .then(|| SignatureControl::new(app, request.kind, accounts, &from, web.widget()))
            .flatten();
        if let Some(control) = &signature {
            actions.append(control.widget());
        }
        let reply_draft = app.map(|app| reply_draft(app, request, accounts, &from, &web, &sender));
        if let Some(draft) = &reply_draft {
            actions.append(draft.slot());
            draft_notes.append(draft.notes());
        }
        let importance = app.map(|app| {
            ImportanceControl::new(
                Arc::clone(app) as Arc<dyn ImportanceStore>,
                &request.composition,
            )
        });
        if let Some(control) = &importance {
            actions.append(control.widget());
        }
        let quote_style = QuoteStylePicker::for_composer(request.has_quote, app, web.widget());
        if let Some(picker) = &quote_style {
            actions.append(picker.widget());
        }
        let editor_host = gtk::Box::new(gtk::Orientation::Vertical, 0);
        editor_host.set_accessible_role(AccessibleRole::Group);
        editor_host.set_hexpand(true);
        editor_host.append(web.widget());
        editor_paste::install(&editor_host, web.widget(), paste);
        // Captured once the seeding script returns, so the guard measures the body against what
        // the quote and signature put there rather than against empty.
        let seed = Rc::new(RefCell::new(None));
        content.append(&editor_host);
        let column = ComposerColumn::new(&content, &editor_host, web.widget());
        super::composer_seed::seed_editor(
            &web,
            request,
            &editor_host,
            signature.as_ref(),
            quote_style,
            Rc::clone(&seed),
        );
        let report = reply_draft.as_ref().map(|draft| {
            let draft = Rc::downgrade(draft);
            Rc::new(move |message: String| {
                if let Some(draft) = draft.upgrade() {
                    draft.report(message);
                }
            }) as Rc<dyn Fn(String)>
        });
        super::composer_host::install(web.widget(), Some(&column), report);
        install_drop_target(column.widget(), take_files(PictureAnswer::Ask));
        toolbar.set_content(Some(column.widget()));
        self.root.append(&toolbar);

        send_button.set_sensitive(!recipients::is_empty(&to.text()));
        let sensitive_button = send_button.clone();
        to.connect_changed(move |field| {
            sensitive_button.set_sensitive(!recipients::is_empty(field));
        });
        let fields = ComposerFields {
            request: request.clone(),
            accounts: accounts.to_vec(),
            from,
            to,
            cc,
            bcc,
            subject,
            files,
            importance,
        };
        let guard = Rc::new(DraftGuard::new(
            web.widget().clone(),
            fields.clone(),
            HeaderValues::on_screen(&fields),
            seed,
        ));
        let input = sender.clone();
        let answer = Rc::clone(&guard);
        discard.connect_clicked(move |_| answer.discard(&input));
        if let Some(button) = &pop_out {
            connect_pop_out(button, request, &guard, signature.as_ref(), &sender);
        }
        self.draft.replace(Some(guard));
        connect_send(
            &send_button,
            web.widget(),
            &fields,
            error.clone(),
            sender.clone(),
            reply_draft.clone(),
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
        self.column.replace(Some(column));
        self.reply_draft.replace(reply_draft);
    }

    /// Reads Draft a reply again after the core signalled it may have moved.
    pub(crate) fn follow_reply_draft(&self, read: u64) {
        if let Some(draft) = self.reply_draft.borrow().as_ref() {
            draft.follow_read(read);
        }
    }

    /// Draws what the draft's last save said (`composer_drafts::draw_draft_hint`).
    pub(crate) fn show_draft_hint(&self, model: &super::AppModel, host: ComposerHost) {
        if let Some(label) = self.hint.borrow().as_ref() {
            super::composer_drafts::draw_draft_hint(label, model.draft_status_of(host));
        }
    }

    /// Lists the signature library again in the picker when the core signalled a read of it.
    pub(crate) fn follow_signatures(&self, read: u64) {
        if let Some(control) = self.signature.borrow().as_ref() {
            control.follow(read);
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

    /// Leaves the open draft for a navigation, once per navigation: saved and closed when it was
    /// written in, closed when it was not (`docs/drafts.md`).
    ///
    /// The generation guard is what makes it once: `render` runs on every update, and the model
    /// cannot clear the request itself because it renders behind a shared reference.
    pub(crate) fn check_draft(&self, generation: u64, sender: &relm4::Sender<AppInput>) {
        if self.checked_generation.get() == Some(generation) {
            return;
        }
        self.checked_generation.set(Some(generation));
        let guard = self.draft.borrow().clone();
        match guard {
            Some(draft) => draft.leave(sender),
            // Nothing to keep; the pane is torn down or was never shown.
            None => sender.emit(AppInput::ComposerUntouched(ComposerHost::Pane)),
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
        self.column.take();
        self.reply_draft.take();
        self.checked_generation.set(None);
        while let Some(child) = self.root.first_child() {
            self.root.remove(&child);
        }
        self.active_generation.set(None);
        self.error.replace(None);
        self.send.replace(None);
    }
}

/// Draft a reply for this composer, sending from what its From picker shows; it follows a change
/// of From itself.
fn reply_draft(
    app: &Arc<MailcalApp>,
    request: &ComposeContext,
    accounts: &[(String, String)],
    from: &gtk::DropDown,
    web: &SecureWebView,
    sender: &relm4::Sender<AppInput>,
) -> Rc<ReplyDraft> {
    let ids = accounts
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    let picker = from.clone();
    let draft = ReplyDraft::new(
        Arc::clone(app) as Arc<dyn ReplyDraftCore>,
        &request.composition,
        Rc::new(move || {
            usize::try_from(picker.selected())
                .ok()
                .and_then(|index| ids.get(index))
                .cloned()
        }),
        web.widget(),
        sender.clone(),
    );
    let following = Rc::downgrade(&draft);
    from.connect_selected_notify(move |_| {
        if let Some(draft) = following.upgrade() {
            draft.follow();
        }
    });
    draft
}

#[cfg(test)]
mod tests {
    use super::EDITOR_HTML;

    #[test]
    fn editor_has_an_accessible_name_before_host_localization_runs() {
        assert!(EDITOR_HTML.contains("aria-label=\"Write your message\""));
    }
}
