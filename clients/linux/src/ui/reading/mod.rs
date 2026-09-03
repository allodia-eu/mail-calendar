//! The Linux reading pane: headers, hardened HTML, remote-content choice, and attachments.

use std::{cell::Cell, sync::Arc};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{
    AttachmentRow, CalendarWriteStatus, MailcalApp, ReadingSnapshot, render_message_html,
};

pub(crate) mod attachments;
pub(crate) mod canvas;

use self::{attachments::attachment_row, canvas as reading_canvas};
use super::{
    AppInput,
    avatar::{AvatarData, Slot as AvatarSlot},
    invitation::InvitationCardView,
    mail_actions::ActionKind,
    model::{OpenedMessage, ReadingState},
    timestamps,
    webview::{DocumentKind, SecureWebView},
};
use crate::l10n;

/// What the invitation card needs from the rest of the app to localise a meeting's times, and what
/// its respond row reports.
///
/// One struct rather than four parameters because these facts travel together; the clock is the
/// *setting*, never the locale's default, so mail and calendar cannot disagree about whether it is
/// 14:05 or 2:05 PM (`docs/timestamps.md`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct InvitationClock<'a> {
    pub(crate) zone: &'a str,
    pub(crate) use_24_hour: bool,
    pub(crate) write_status: CalendarWriteStatus,
    /// Which reading snapshot this is. Every model change re-renders the pane, and rebuilding the
    /// card on a *write-status* change would take a half-typed note to the organiser away
    /// mid-sentence; so the card is rebuilt only when the core published a new snapshot, and
    /// every other render moves the respond row alone.
    pub(crate) generation: u64,
}

pub(crate) struct ReadingPane {
    root: gtk::Box,
    subject: gtk::Label,
    avatar: AvatarSlot,
    from: gtk::Label,
    recipients: gtk::Label,
    date: gtk::Label,
    actions: [gtk::Button; 5],
    remote_banner: adw::Banner,
    /// The meeting-invitation card, above the body. Whether there is one at all is the core's
    /// two-condition RSVP gate (`docs/invitations.md`), so a published `.ics` produces none here
    /// and keeps its attachment chip instead.
    invitation: InvitationCardView,
    /// Which snapshot the drawn card belongs to: see [`InvitationClock::generation`].
    invitation_generation: Cell<Option<u64>>,
    body_stack: gtk::Stack,
    spinner: gtk::Spinner,
    plain: gtk::Label,
    empty: gtk::Label,
    attachments: gtk::Box,
    web: SecureWebView,
    window: adw::ApplicationWindow,
}

impl ReadingPane {
    pub(crate) fn new(window: &adw::ApplicationWindow, sender: relm4::Sender<AppInput>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_show_start_title_buttons(false);
        let reply = action_button("mail-reply-sender-symbolic", l10n::action_reply());
        let reply_all = action_button("mail-reply-all-symbolic", l10n::action_reply_all());
        let forward = action_button("mail-forward-symbolic", l10n::action_forward());
        let archive = action_button("mailcal-archive-symbolic", l10n::action_archive());
        let trash = action_button("user-trash-symbolic", l10n::action_move_to_trash());
        let input_sender = sender.clone();
        reply.connect_clicked(move |_| input_sender.emit(AppInput::BeginReply(false)));
        header.pack_start(&reply);
        let input_sender = sender.clone();
        reply_all.connect_clicked(move |_| input_sender.emit(AppInput::BeginReply(true)));
        header.pack_start(&reply_all);
        let input_sender = sender.clone();
        forward.connect_clicked(move |_| input_sender.emit(AppInput::BeginForward));
        header.pack_start(&forward);
        let input_sender = sender.clone();
        archive.connect_clicked(move |_| {
            input_sender.emit(AppInput::PerformOpenedMailAction(ActionKind::Archive));
        });
        header.pack_end(&archive);
        let input_sender = sender.clone();
        trash.connect_clicked(move |_| {
            input_sender.emit(AppInput::PerformOpenedMailAction(ActionKind::MoveToTrash));
        });
        header.pack_end(&trash);
        toolbar.add_top_bar(&header);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
        content.set_margin_top(18);
        content.set_margin_bottom(18);
        content.set_margin_start(18);
        content.set_margin_end(18);
        let subject = heading_label();
        let avatar = AvatarSlot::new(48);
        let from = metadata_label();
        let recipients = metadata_label();
        let date = metadata_label();
        content.append(&subject);
        let identity = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        identity.append(avatar.widget());
        let metadata = gtk::Box::new(gtk::Orientation::Vertical, 2);
        metadata.set_hexpand(true);
        metadata.append(&from);
        metadata.append(&recipients);
        metadata.append(&date);
        identity.append(&metadata);
        content.append(&identity);

        let remote_banner = adw::Banner::new(l10n::reading_remote_blocked());
        remote_banner.set_button_label(Some(l10n::action_load_images()));
        let input_sender = sender.clone();
        remote_banner
            .connect_button_clicked(move |_| input_sender.emit(AppInput::LoadRemoteImages));
        content.append(&remote_banner);

        let invitation = InvitationCardView::new();
        content.append(invitation.widget());

        reading_canvas::install_styles();
        let body_stack = gtk::Stack::new();
        body_stack.set_vexpand(true);
        let spinner = gtk::Spinner::new();
        spinner.set_spinning(true);
        body_stack.add_named(&spinner, Some("loading"));
        // An open the core has not yet called slow: no spinner, no words. A stored body arrives
        // in milliseconds, and drawing one on every open flickers instead of reassuring. The page
        // underneath stays, an empty sheet rather than a hole ([`reading_canvas`]).
        body_stack.add_named(&gtk::Box::new(gtk::Orientation::Vertical, 0), Some("blank"));
        let web = SecureWebView::new(DocumentKind::Reading, sender.clone());
        body_stack.add_named(web.widget(), Some("html"));
        let plain = gtk::Label::new(None);
        plain.set_wrap(true);
        plain.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        plain.set_xalign(0.0);
        plain.set_yalign(0.0);
        plain.set_selectable(true);
        let plain_scroll = gtk::ScrolledWindow::new();
        plain_scroll.set_child(Some(&plain));
        body_stack.add_named(&plain_scroll, Some("plain"));
        let empty = gtk::Label::new(None);
        empty.set_wrap(true);
        body_stack.add_named(&empty, Some("empty"));
        // Nothing open: the pane is waiting to be given a message, so it draws no page at all.
        let idle = gtk::Label::new(Some(l10n::reading_empty()));
        idle.set_wrap(true);
        body_stack.add_named(&idle, Some("idle"));
        let error = gtk::Box::new(gtk::Orientation::Vertical, 8);
        error.set_valign(gtk::Align::Center);
        error.append(&gtk::Label::new(Some(l10n::reading_load_error())));
        let retry = gtk::Button::with_label(l10n::action_retry());
        let input_sender = sender;
        retry.connect_clicked(move |_| input_sender.emit(AppInput::RetryOpen));
        error.append(&retry);
        body_stack.add_named(&error, Some("error"));
        content.append(&body_stack);

        let attachments = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.append(&attachments);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_child(Some(&content));
        toolbar.set_content(Some(&scroll));
        root.append(&toolbar);

        Self {
            root,
            subject,
            avatar,
            from,
            recipients,
            date,
            actions: [reply, reply_all, forward, archive, trash],
            remote_banner,
            invitation,
            invitation_generation: Cell::new(None),
            body_stack,
            spinner,
            plain,
            empty,
            attachments,
            web,
            window: window.clone(),
        }
    }

    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub(crate) fn suspend(&self) {
        self.web.clear();
    }

    pub(crate) fn render(
        &self,
        state: &ReadingState,
        app: Option<&Arc<MailcalApp>>,
        webview_available: bool,
        clock: InvitationClock<'_>,
        sender: &relm4::Sender<AppInput>,
    ) {
        for action in &self.actions {
            action.set_sensitive(state.opened.is_some());
        }
        let Some(opened) = state.opened.as_ref() else {
            self.clear_header();
            self.remote_banner.set_revealed(false);
            self.clear_invitation();
            self.show("idle");
            return;
        };
        self.subject.set_text(if opened.subject.trim().is_empty() {
            l10n::mail_no_subject()
        } else {
            &opened.subject
        });
        self.from.set_text(&opened.from);
        self.date
            .set_text(&timestamps::local_date_time(&opened.date, clock.zone));
        self.avatar.set(&opened.avatar);
        if !state.matches_opened() {
            self.recipients.set_text("");
            self.remote_banner.set_revealed(false);
            // A stale snapshot belongs to the message that was open a moment ago; drawing its card
            // over the one now loading would offer an answer to the wrong meeting.
            self.clear_invitation();
            self.render_attachments(&[], sender);
            self.show("blank");
            return;
        }
        let reading = &state.snapshot;
        self.avatar.set(&AvatarData::from(&reading.avatar));
        match reading.invitation.as_ref() {
            Some(card) if self.invitation_generation.get() != Some(clock.generation) => {
                self.invitation_generation.set(Some(clock.generation));
                self.invitation.apply(
                    card,
                    clock.zone,
                    clock.use_24_hour,
                    clock.write_status,
                    sender,
                );
            }
            // The same card, re-rendered for something else entirely; report the write settling
            // and leave the note alone.
            Some(_) => self.invitation.set_write_status(clock.write_status),
            None => self.clear_invitation(),
        }
        self.from.set_text(sender_line(reading, opened));
        self.recipients.set_text(&recipient_line(reading));
        self.render_attachments(&reading.attachments, sender);
        if reading.pending {
            // The core publishes this only once an open has run long enough to be worth
            // announcing. It carries no body, so it has to be read before the branches that
            // look for one: otherwise a wait draws the "no content" page.
            self.remote_banner.set_revealed(false);
            self.show("loading");
        } else if reading.load_error {
            self.remote_banner.set_revealed(false);
            self.show("error");
        } else if let Some(html) = reading.html.as_deref() {
            if webview_available && app.is_some() {
                self.remote_banner
                    .set_revealed(reading.has_remote_images && !state.load_remote_images);
                self.web.load(
                    &render_message_html(html.to_owned(), state.load_remote_images),
                    state.load_remote_images,
                );
                // Hold the page until the document is actually on it. The view composites on a
                // surface of its own that arrives black, so revealing it the moment a load
                // starts puts a black rectangle in the middle of the pane for as long as the
                // message takes to lay out ([`SecureWebView::painted`]), with the canvas either
                // side of it, which is what makes it read as a flash rather than as loading.
                self.show(if self.web.painted() { "html" } else { "blank" });
            } else {
                self.show_fallback(reading);
            }
        } else if let Some(plain) = reading.plain.as_deref() {
            self.remote_banner.set_revealed(false);
            self.plain.set_text(plain);
            self.show("plain");
        } else {
            self.remote_banner.set_revealed(false);
            self.empty.set_text(l10n::reading_no_content());
            self.show("empty");
        }
    }

    fn show_fallback(&self, reading: &ReadingSnapshot) {
        self.remote_banner.set_revealed(false);
        if let Some(plain) = reading.plain.as_deref() {
            self.plain.set_text(plain);
            self.show("plain");
        } else {
            self.empty.set_text(l10n::reading_webview_unavailable());
            self.show("empty");
        }
    }

    fn show(&self, name: &str) {
        self.spinner.set_spinning(name == "loading");
        // Every page but `idle` is a message's body area, so all of them sit on the same sheet
        // and an open changes what is written on the page rather than the page itself.
        reading_canvas::set_drawn(&self.body_stack, name != "idle");
        self.body_stack.set_visible_child_name(name);
    }

    fn clear_invitation(&self) {
        self.invitation_generation.set(None);
        self.invitation.clear();
    }

    fn clear_header(&self) {
        self.subject.set_text("");
        self.avatar.clear();
        self.from.set_text("");
        self.recipients.set_text("");
        self.date.set_text("");
        while let Some(child) = self.attachments.first_child() {
            self.attachments.remove(&child);
        }
    }

    fn render_attachments(&self, rows: &[AttachmentRow], sender: &relm4::Sender<AppInput>) {
        while let Some(child) = self.attachments.first_child() {
            self.attachments.remove(&child);
        }
        if rows.is_empty() {
            return;
        }
        let title = gtk::Label::new(Some(l10n::attachments_title()));
        title.set_xalign(0.0);
        title.add_css_class("heading");
        self.attachments.append(&title);
        for attachment in rows {
            self.attachments
                .append(&attachment_row(attachment, &self.window, sender));
        }
    }
}

fn action_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.set_tooltip_text(Some(tooltip));
    button.update_property(&[AccessibleProperty::Label(tooltip)]);
    button
}

fn heading_label() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class("title-2");
    label
}

fn metadata_label() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class("dim-label");
    label
}

fn recipient_line(reading: &ReadingSnapshot) -> String {
    [
        (!reading.to.is_empty()).then(|| format!("{}: {}", l10n::compose_to(), reading.to)),
        (!reading.cc.is_empty()).then(|| format!("{}: {}", l10n::compose_cc(), reading.cc)),
        (!reading.bcc.is_empty()).then(|| format!("{}: {}", l10n::compose_bcc(), reading.bcc)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n")
}

fn sender_line<'a>(reading: &'a ReadingSnapshot, opened: &'a OpenedMessage) -> &'a str {
    if reading.from.trim().is_empty() {
        &opened.from
    } else {
        &reading.from
    }
}

#[cfg(test)]
#[path = "tests.rs"]
pub(crate) mod attachment_tests;

#[cfg(test)]
mod tests {
    use mailcal_bindings::ReadingSnapshot;

    use super::sender_line;
    use crate::ui::model::OpenedMessage;

    #[test]
    fn loaded_sender_uses_the_full_reading_header_and_loading_falls_back_to_the_row() {
        let opened = OpenedMessage {
            account: "account".to_owned(),
            key: "message".to_owned(),
            subject: "Subject".to_owned(),
            from: "Sender".to_owned(),
            date: "2026-07-20".to_owned(),
            avatar: crate::ui::avatar::AvatarData::from(&crate::ui::model::blank_avatar()),
        };
        let mut reading = ReadingSnapshot {
            avatar: crate::ui::model::blank_avatar(),
            key: "message".to_owned(),
            from: "Sender <sender@example.test>".to_owned(),
            to: String::new(),
            cc: String::new(),
            bcc: String::new(),
            html: None,
            plain: None,
            has_remote_images: false,
            load_error: false,
            attachments: Vec::new(),
            invitation: None,
            pending: false,
        };

        assert_eq!(
            sender_line(&reading, &opened),
            "Sender <sender@example.test>"
        );
        reading.from.clear();
        assert_eq!(sender_line(&reading, &opened), "Sender");
    }
}
