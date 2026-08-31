//! The meeting-invitation card, drawn **above** the message body; the Linux twin of
//! `InvitationCardView.swift`, `InvitationCard.kt` and `InvitationCardView.cs`.
//!
//! Everything on it was decided by the core (`docs/invitations.md`): whether there is a card at
//! all, the organiser line, the attendee tally, the conflict count and the preview's geometry.
//! This view localises and arranges; it computes no counts of its own, so this client and the next
//! cannot disagree about whether a meeting clashes.
//!
//! **Security (Gate 8, `docs/rendering-security.md`).** `summary`, `location`, `description` and
//! the organiser's name are attacker-controlled sender content, and they reach the screen without
//! passing the HTML sanitiser, the CSP or a web view. On GTK a plain string is *not* safe by
//! default; a libadwaita row parses its title **and subtitle** as Pango markup unless told
//! otherwise, and a `GtkLabel` parses it whenever `use-markup` is on. So every untrusted field here
//! goes through `untrusted_label`, which states `use_markup(false)` rather than trusting a default,
//! and the one libadwaita row on the card carries only our own localised string. Nothing here
//! reaches the composer bridge or a `WebKitWebView`.
//!
//! The conflict count is stated in **words** beside the preview grid, always: `docs/calendar.md`
//! §4: a picture the user has to read carefully is not a disclosure.

use std::cell::RefCell;

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{CalendarWriteStatus, InvitationCard, InvitationKind, InvitationResponse};

use super::{
    attendees, conflicts, notice, preview::PreviewGrid, reply_subject, response, title, when,
    write_line,
};
use crate::{l10n, ui::AppInput};

/// What a press on one of the three answers carries back to the model.
///
/// The **message** is named by the caller, never the event: the answer goes out as the address the
/// invitation matched, which on an aliased account is not the account's primary identity, and only
/// the core knows the address set (`docs/invitations.md` §4).
#[derive(Clone, Debug)]
pub(crate) struct InvitationAnswer {
    pub(crate) response: InvitationResponse,
    /// A note for the organiser: `None` where the transport carries none. Sending one a transport
    /// cannot carry fails the whole answer rather than quietly losing the text, so this is `None`
    /// unless the card said `can_comment`.
    pub(crate) comment: Option<String>,
    pub(crate) notify_organizer: bool,
    /// The localised subject for the reply the core may have to email itself.
    pub(crate) reply_subject: String,
}

/// The card for the open message: who is asking, when, and what it clashes with.
pub(crate) struct InvitationCardView {
    root: gtk::Frame,
    body: gtk::Box,
    preview: PreviewGrid,
    /// The controls a settling write takes out of reach together, rebuilt with the card.
    answers: RefCell<Vec<gtk::Widget>>,
    /// What happened to the answer; hidden unless there is something to say.
    write_status: gtk::Label,
}

impl InvitationCardView {
    pub(crate) fn new() -> Self {
        let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
        body.set_margin_top(10);
        body.set_margin_bottom(10);
        body.set_margin_start(10);
        body.set_margin_end(10);
        let root = gtk::Frame::new(None);
        root.set_child(Some(&body));
        root.set_visible(false);
        let write_status = gtk::Label::new(None);
        write_status.set_xalign(0.0);
        write_status.set_wrap(true);
        write_status.set_visible(false);
        Self {
            root,
            body,
            preview: PreviewGrid::new(),
            answers: RefCell::new(Vec::new()),
            write_status,
        }
    }

    pub(crate) fn widget(&self) -> &gtk::Frame {
        &self.root
    }

    /// Clears the card; the open message carries no invitation, or none has loaded yet.
    pub(crate) fn clear(&self) {
        self.root.set_visible(false);
        self.answers.borrow_mut().clear();
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
    }

    /// Draws `card`, rebuilt whole.
    ///
    /// `zone` is the display zone: the card's instants are UTC and the host localises them
    /// (`docs/timestamps.md`). `use_24_hour` is the app's clock **setting** rather than the
    /// locale's default, so mail and calendar cannot disagree about whether it is 14:05 or 2:05 PM.
    pub(crate) fn apply(
        &self,
        card: &InvitationCard,
        zone: &str,
        use_24_hour: bool,
        status: CalendarWriteStatus,
        sender: &relm4::Sender<AppInput>,
    ) {
        self.clear();
        self.root.set_visible(true);
        self.body.append(&heading(card.kind));
        if let Some(notice) = notice(card.kind) {
            self.body.append(&caption(notice));
        }
        let summary = if card.summary.trim().is_empty() {
            l10n::invitation_no_title().to_owned()
        } else {
            card.summary.clone()
        };
        let title = untrusted_label(&summary);
        title.add_css_class("heading");
        self.body.append(&title);

        self.body
            .append(&detail(l10n::invitation_organizer(), &card.organizer));
        self.body.append(&detail(
            l10n::invitation_when(),
            &when(
                &card.starts_at,
                &card.ends_at,
                card.all_day,
                zone,
                use_24_hour,
            ),
        ));
        if !card.location.is_empty() {
            self.body
                .append(&detail(l10n::invitation_where(), &card.location));
        }
        if card.recurring {
            self.body.append(&caption(l10n::invitation_repeats()));
        }
        self.append_description(card);
        self.append_answer(card, &summary, sender);
        self.append_conflicts(card, zone, use_24_hour);
        // A direct child of the card's box, so `clear` unparents it through `GtkBox::remove` like
        // every other row; a status line nested in the respond row would still be parented to a
        // discarded box when the next card tried to claim it.
        self.body.append(&self.write_status);
        self.set_write_status(status);
    }

    /// The organiser's notes. Already truncated by the core (Gmail sends a wall of filler), and the
    /// card says so rather than implying the text ends there.
    fn append_description(&self, card: &InvitationCard) {
        if card.description.is_empty() {
            return;
        }
        let notes = untrusted_label(&card.description);
        notes.add_css_class("dim-label");
        notes.set_selectable(true);
        self.body.append(&notes);
        if card.description_truncated {
            self.body
                .append(&caption(l10n::invitation_description_shortened()));
        }
    }

    /// This account's own answer, how everyone else answered, and the buttons to change it.
    ///
    /// Both lines read the **calendar's** copy, not the email's; the mail is frozen at the moment
    /// it was sent, so a card built from it would still say "you haven't answered" after you had,
    /// and would go on counting you among the people yet to reply. Only a card carrying an RSVP
    /// shows buttons: a cancellation has nothing to answer.
    fn append_answer(
        &self,
        card: &InvitationCard,
        summary: &str,
        sender: &relm4::Sender<AppInput>,
    ) {
        self.body
            .append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let answered = gtk::Label::new(Some(response(card.my_response)));
        answered.set_xalign(0.0);
        answered.set_wrap(true);
        self.body.append(&answered);
        let tally = attendees(&card.attendees);
        if !tally.is_empty() {
            self.body.append(&caption(&tally));
        }
        if matches!(card.kind, InvitationKind::Rsvp) {
            self.append_respond_row(card, summary, sender);
        }
    }

    /// What else is in the calendar then; stated in words, then shown.
    ///
    /// The preview is offered only when the calendar was actually read. An empty grid drawn over an
    /// unread calendar looks exactly like a free day, which is the whole failure this guards.
    fn append_conflicts(&self, card: &InvitationCard, zone: &str, use_24_hour: bool) {
        let line = gtk::Label::new(Some(&conflicts(card.conflict_count, card.conflicts_known)));
        line.set_xalign(0.0);
        line.set_wrap(true);
        line.set_margin_top(4);
        if !(card.conflicts_known && card.conflict_count > 0) {
            line.add_css_class("dim-label");
        }
        self.body.append(&line);
        if !card.conflicts_known {
            return;
        }
        self.preview.apply(
            &card.preview,
            &card.starts_at,
            &card.ends_at,
            zone,
            use_24_hour,
        );
        let disclosure = adw::ExpanderRow::builder()
            // "Around this meeting", never "your calendar that day": the band is the meeting and
            // what overlaps it, and the label says which picture this is.
            .title(l10n::invitation_conflicts_preview())
            .use_markup(false)
            // Open whenever the calendar was actually read; which the early return above has
            // already established, so this is unconditional. Not "open when the count is non-zero":
            // the question a person answering an invitation asks is "what does my day look like",
            // and the answer to that is the picture, not the number.
            .expanded(true)
            .build();
        let holder = adw::ActionRow::new();
        holder.set_activatable(false);
        holder.set_child(Some(self.preview.widget()));
        disclosure.add_row(&holder);
        let group = adw::PreferencesGroup::new();
        group.add(&disclosure);
        group.set_margin_top(4);
        self.body.append(&group);
    }

    /// Reports the write currently settling, without rebuilding the card; a rebuild would take a
    /// half-typed note to the organiser away mid-sentence.
    pub(crate) fn set_write_status(&self, status: CalendarWriteStatus) {
        let settling = matches!(status, CalendarWriteStatus::Saving);
        for control in self.answers.borrow().iter() {
            control.set_sensitive(!settling);
        }
        match write_line(status) {
            Some(line) => {
                self.write_status.set_text(line);
                self.write_status.set_visible(true);
            }
            None => self.write_status.set_visible(false),
        }
        if matches!(status, CalendarWriteStatus::Failed) {
            self.write_status.add_css_class("error");
        } else {
            self.write_status.remove_css_class("error");
        }
    }
}

fn heading(kind: InvitationKind) -> gtk::Label {
    let heading = gtk::Label::new(Some(title(kind)));
    heading.set_xalign(0.0);
    heading.set_wrap(true);
    heading.add_css_class("caption-heading");
    // A heading, so a screen reader's heading navigation lands on the card in one hop. The card
    // itself carries NO container label: naming the container is what collapsed the whole card into
    // a single node on iOS and put the three buttons out of reach of VoiceOver. Every line here is
    // reachable on its own instead.
    heading.set_accessible_role(gtk::AccessibleRole::Heading);
    heading.update_property(&[AccessibleProperty::Level(3)]);
    match kind {
        // A cancellation has to be unmissable; a stale hold otherwise sits in the calendar looking
        // like a commitment.
        InvitationKind::Cancelled => heading.add_css_class("error"),
        // Caution, not error: nothing was lost, there is simply a newer copy to open.
        InvitationKind::Superseded => heading.add_css_class("warning"),
        InvitationKind::Informational => heading.add_css_class("dim-label"),
        InvitationKind::Rsvp => heading.add_css_class("accent"),
    }
    heading
}

/// One `label: value` row, with the value rendered as **text**.
fn detail(label: &str, value: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let name = caption(label);
    name.set_size_request(76, -1);
    name.set_valign(gtk::Align::Start);
    row.append(&name);
    let text = untrusted_label(value);
    // An organiser's address and a meeting room are things people copy out.
    text.set_selectable(true);
    text.set_hexpand(true);
    row.append(&text);
    row
}

fn caption(text: &str) -> gtk::Label {
    let caption = gtk::Label::new(Some(text));
    caption.set_xalign(0.0);
    caption.set_wrap(true);
    caption.add_css_class("dim-label");
    caption
}

/// A label for **attacker-controlled** sender content.
///
/// **`set_text` is what makes it safe**: it takes the string as text and clears `use-markup` on
/// the way: and `set_markup` is the one-word refactor that undoes it: the fields here carry
/// ampersands, so a marked-up label renders **blank** and a markup-shaped location arrives styled
/// (`docs/rendering-security.md`, Gate 8). The explicit `set_use_markup(false)` says the intent out
/// loud beside it, because the libadwaita rows elsewhere in this client parse their title *and*
/// subtitle as markup by default and a reader has no reason to assume a `GtkLabel` differs.
fn untrusted_label(value: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_use_markup(false);
    label.set_text(value);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label
}

// Accept / Maybe / Decline, and the two controls that ride beside them on the transports that have
// them. A child module rather than a sibling so it reaches this struct's private fields; the same
// seam Windows draws with a `partial class`.
mod respond;
