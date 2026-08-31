//! Widget-level regressions for the invitation card and the reply-undelivered question.
//!
//! Called from the crate's one GTK test (`ui::mailbox_tests`) because GTK may be initialised once
//! per process. What is worth asserting here is the half neither a unit test nor a screenshot can
//! reach: what the widgets actually *render*, whether a control is present at all, and; the trap
//! this platform sets; whether an untrusted field was parsed as Pango markup on the way.

use adw::prelude::*;
use mailcal_bindings::{
    AttendeeTally, CalendarWriteStatus, InvitationCard, InvitationKind, InvitationPreview,
    InvitationResponse, ReplyPrompt, ResponseStatus,
};

use super::{InvitationCardView, ReplyPromptDialog};
use crate::{
    l10n,
    ui::{
        AppInput,
        mailbox::tests::{glib_records, labels, rendered_labels},
    },
};

const ZONE: &str = "Europe/Amsterdam";

fn card() -> InvitationCard {
    InvitationCard {
        kind: InvitationKind::Rsvp,
        organizer: "Allodia Mail & Calendar <bob@example.test>".to_owned(),
        summary: "Research & Development".to_owned(),
        location: "<b>Room 4</b> & the corridor".to_owned(),
        description: "Budget & headcount".to_owned(),
        description_truncated: false,
        starts_at: "2026-08-17T08:30:00Z".to_owned(),
        ends_at: "2026-08-17T09:30:00Z".to_owned(),
        all_day: false,
        recurring: false,
        my_response: ResponseStatus::NeedsAction,
        attendees: AttendeeTally {
            total: 3,
            accepted: 2,
            declined: 0,
            tentative: 0,
            needs_action: 1,
        },
        conflict_count: 2,
        conflicts_known: true,
        preview: InvitationPreview {
            days: Vec::new(),
            timed: Vec::new(),
            all_day: Vec::new(),
            all_day_lanes: 0,
            timezone: ZONE.to_owned(),
        },
        can_respond: true,
        can_comment: false,
        can_choose_notify: false,
    }
}

/// The strings the card put on screen, for `card`.
fn shown_for(card: &InvitationCard) -> (Vec<String>, Vec<String>) {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let view = InvitationCardView::new();
    let ((), records) = glib_records(|| {
        view.apply(card, ZONE, true, CalendarWriteStatus::Idle, &sender);
    });
    let shown = rendered_labels(view.widget().clone().upcast_ref::<gtk::Widget>());
    (shown, records)
}

/// **Gate 8, `docs/rendering-security.md`.** Every field here came from whoever sent the mail.
///
/// Both halves have to be checked and neither sees the other's defect: the *rendering* assertion
/// catches a field parsed as markup (a bare ampersand renders the label **blank**, and a
/// markup-shaped subject arrives styled), and the *log* assertion catches the ordering bug that
/// renders correctly anyway: libadwaita re-applies its labels when `use-markup` flips, leaving
/// only a `Failed to set text … from markup` warning per field, in the diagnostic log a user
/// attaches to a support request.
pub(crate) fn an_invitations_own_text_is_never_parsed_as_markup() {
    let (shown, records) = shown_for(&card());
    assert!(
        !records.iter().any(|line| line.contains("from markup")),
        "the card must not parse the organizer's text as markup: {records:?}"
    );
    for verbatim in [
        "Research & Development",
        "Allodia Mail & Calendar <bob@example.test>",
        "<b>Room 4</b> & the corridor",
        "Budget & headcount",
    ] {
        assert!(
            shown.iter().any(|text| text == verbatim),
            "{verbatim:?} must render as itself, never blank and never styled: {shown:?}"
        );
    }
}

/// An account that cannot deliver a response says so, rather than offering a control that lies.
pub(crate) fn an_account_that_cannot_answer_says_so_instead_of_greying_the_buttons() {
    let mut refused = card();
    refused.can_respond = false;
    let (shown, _) = shown_for(&refused);
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::invitation_cannot_respond()),
        "a card with no route to the organizer must explain itself: {shown:?}"
    );
    assert!(
        !shown.iter().any(|text| text == l10n::invitation_accept()),
        "absent with an explanation, never present and disabled: {shown:?}"
    );
}

/// The note and the tick appear **only** where the transport carries them: the core refuses a note
/// it cannot carry rather than dropping it, so an offered field would lose the whole answer.
pub(crate) fn the_note_and_the_tick_appear_only_where_the_transport_carries_them() {
    let (bare, _) = shown_for(&card());
    assert!(
        !bare
            .iter()
            .any(|text| text == l10n::invitation_notify_organizer()),
        "a server-scheduled transport cannot be told to stay quiet: {bare:?}"
    );

    let mut generous = card();
    generous.can_comment = true;
    generous.can_choose_notify = true;
    let (shown, _) = shown_for(&generous);
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::invitation_notify_organizer()),
        "the tick belongs on a transport that honours it: {shown:?}"
    );
    // The note is a placeholder rather than a label, so it is asserted on the entry itself.
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let view = InvitationCardView::new();
    view.apply(&generous, ZONE, true, CalendarWriteStatus::Idle, &sender);
    assert!(
        entries(view.widget().clone().upcast_ref::<gtk::Widget>())
            .iter()
            .any(|entry| entry.placeholder_text().is_some_and(|placeholder| {
                placeholder == l10n::invitation_message_to_organizer()
            })),
        "a transport that carries a note must offer the field"
    );
}

/// A cancellation and a superseded copy show every detail and offer no answer; and the superseded
/// one **says why**, or it reads as broken rather than out of date.
pub(crate) fn a_cancelled_or_superseded_card_states_itself_and_offers_no_answer() {
    for (kind, heading) in [
        (
            InvitationKind::Cancelled,
            l10n::invitation_cancelled_title(),
        ),
        (
            InvitationKind::Superseded,
            l10n::invitation_superseded_title(),
        ),
        (
            InvitationKind::Informational,
            l10n::invitation_informational_title(),
        ),
    ] {
        let mut other = card();
        other.kind = kind;
        let (shown, _) = shown_for(&other);
        assert!(shown.iter().any(|text| text == heading), "{shown:?}");
        assert!(
            !shown.iter().any(|text| text == l10n::invitation_accept()),
            "only an RSVP offers an answer: {shown:?}"
        );
        assert!(
            shown.iter().any(|text| text == "Research & Development"),
            "the details stay on screen whatever the kind: {shown:?}"
        );
    }
    let mut superseded = card();
    superseded.kind = InvitationKind::Superseded;
    let (shown, _) = shown_for(&superseded);
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::invitation_superseded()),
        "an out-of-date card has to say why it offers nothing: {shown:?}"
    );
}

/// An unread calendar is not an empty one: the count is withheld **and so is the picture**, because
/// an empty grid over a calendar nobody read is indistinguishable from a free day.
pub(crate) fn an_unread_calendar_withholds_both_the_count_and_the_grid() {
    let mut unread = card();
    unread.conflicts_known = false;
    let (shown, _) = shown_for(&unread);
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::invitation_conflicts_unknown()),
        "say we have not looked: {shown:?}"
    );
    assert!(
        !shown
            .iter()
            .any(|text| text == l10n::invitation_conflicts_preview()),
        "no disclosure over a calendar nobody read: {shown:?}"
    );

    let (read, _) = shown_for(&card());
    assert!(
        read.iter()
            .any(|text| text == l10n::invitation_conflicts_preview()),
        "and the grid is open the moment the calendar was read: {read:?}"
    );
}

/// The one write state that must never be silent, and the one that must be.
pub(crate) fn a_settling_write_is_reported_and_a_settled_one_is_not() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let view = InvitationCardView::new();
    let card = card();
    view.apply(&card, ZONE, true, CalendarWriteStatus::Idle, &sender);
    let root = view.widget().clone().upcast_ref::<gtk::Widget>().clone();

    view.set_write_status(CalendarWriteStatus::Failed);
    assert!(
        visible_labels(&root)
            .iter()
            .any(|text| text == l10n::invitation_failed()),
        "a reply the organizer never received, reported as sent, is the failure this exists to stop"
    );
    // Saved says nothing on purpose: the card has been rebuilt from the calendar by then and
    // already shows the new answer.
    view.set_write_status(CalendarWriteStatus::Saved);
    assert!(
        !visible_labels(&root)
            .iter()
            .any(|text| text == l10n::invitation_failed())
    );
}

/// The question a calendar server raises when it stored the answer and could not pass it on.
///
/// Three of the four rules are visible in one render: the RSVP is reported as fine, the recipient
/// is **named**, and the RFC 6638 status code is **not** on screen.
pub(crate) fn the_reply_question_names_the_recipient_and_withholds_the_status_code() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let application = adw::Application::builder()
        .application_id(format!("{}.invitation-test", crate::l10n::APP_ID))
        .build();
    application
        .register(None::<&gtk::gio::Cancellable>)
        .expect("register test application");
    let window = adw::ApplicationWindow::new(&application);
    let dialog = ReplyPromptDialog::new();
    let prompt = ReplyPrompt {
        account: "fixture".to_owned(),
        summary: "Research & Development".to_owned(),
        organizer: "bob@example.test".to_owned(),
        response: InvitationResponse::Decline,
        status_code: "5.2".to_owned(),
    };
    let ((), records) = glib_records(|| dialog.render(Some(&prompt), 1, &window, &sender));
    let modal = dialog.window().expect("the question is on screen");
    let shown = rendered_labels(modal.clone().upcast_ref::<gtk::Widget>());
    assert!(
        !records.iter().any(|line| line.contains("from markup")),
        "the meeting's title and the organizer's address are attacker-controlled: {records:?}"
    );
    assert!(
        shown.iter().any(|text| {
            text.contains("bob@example.test") && text.contains("Research & Development")
        }),
        "consent to send mail on someone's behalf is not informed without the recipient: {shown:?}"
    );
    assert!(
        !shown.iter().any(|text| text.contains("5.2")),
        "the status code is for the log; it explains nothing to the person reading this: {shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|text| text == l10n::invitation_reply_undelivered_remember()),
        "the choice can be remembered, in both directions: {shown:?}"
    );

    // Nothing may dismiss it without answering, or the core goes on holding a question the user
    // can no longer see. Both routes out are refused.
    assert!(!modal.is_deletable());
    let refused: bool = modal.emit_by_name("close-request", &[]);
    assert!(
        refused,
        "the close handler must stop the signal, not merely ignore it"
    );
    assert!(modal.is_visible(), "a refused close must leave it standing");

    // `None` is equally how the core says *close it*: it clears the question the moment it is
    // answered, so a stale window cannot answer twice.
    dialog.render(None, 1, &window, &sender);
    assert!(dialog.window().is_none());
}

fn entries(root: &gtk::Widget) -> Vec<gtk::Entry> {
    let mut found = Vec::new();
    if let Some(entry) = root.downcast_ref::<gtk::Entry>() {
        found.push(entry.clone());
    }
    let mut child = root.first_child();
    while let Some(node) = child {
        found.extend(entries(&node));
        child = node.next_sibling();
    }
    found
}

/// The labels that are actually on screen; a hidden one is not a report.
fn visible_labels(root: &gtk::Widget) -> Vec<String> {
    labels(root)
        .iter()
        .filter(|label| label.is_visible())
        .map(|label| label.text().to_string())
        .collect()
}
