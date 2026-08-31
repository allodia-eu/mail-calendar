//! What a recipient field must draw, and what it must never draw.
//!
//! Driven through the real widget; a keystroke goes in as text and the assertions read the pills,
//! the input and the suggestion list back out. Called from the crate's single `gtk::init` test
//! (see [`crate::ui::mailbox::tests`]).

use adw::prelude::*;
use mailcal_bindings::RecipientMatch;

use super::RecipientField;
use crate::ui::mailbox::tests::{glib_records, rendered_labels};

fn field(label: &str) -> RecipientField {
    // No core: the field still works, it just offers nothing until `show_suggestions` is handed a
    // result. That is also what a client whose core failed to boot gets.
    RecipientField::new(label, None)
}

fn found(name: &str, email: &str) -> RecipientMatch {
    RecipientMatch {
        email: email.to_owned(),
        display_name: name.to_owned(),
        is_saved: false,
    }
}

/// Every `GtkButton` under `root`, in tree order.
fn buttons(root: &gtk::Widget) -> Vec<gtk::Button> {
    let mut found = Vec::new();
    if let Some(button) = root.downcast_ref::<gtk::Button>() {
        found.push(button.clone());
    }
    let mut child = root.first_child();
    while let Some(node) = child {
        found.extend(buttons(&node));
        child = node.next_sibling();
    }
    found
}

fn pill_labels(field: &RecipientField) -> Vec<String> {
    rendered_labels(field.inner.pills.upcast_ref::<gtk::Widget>())
}

fn pill_removes(field: &RecipientField) -> Vec<gtk::Button> {
    buttons(field.inner.pills.upcast_ref::<gtk::Widget>())
}

/// A field the composer was **opened** with has nothing in progress, so every address it carries is
/// a pill and the input is empty. Seeded raw, a reply-all's last recipient sat in the input as
/// half-typed text and a single-address Cc drew no pill at all; the field looked like it had
/// dropped the people it was in fact holding.
pub(crate) fn a_seeded_field_draws_every_address_as_a_pill() {
    let to = field("To");
    to.seed("bestuur@example.test, tc@example.test");

    assert_eq!(
        pill_labels(&to),
        vec!["bestuur@example.test", "tc@example.test"]
    );
    assert_eq!(to.inner.entry.text(), "", "nothing is in progress");
    assert_eq!(to.text(), "bestuur@example.test, tc@example.test, ");
    assert!(to.inner.pills.is_visible());

    // An empty field draws no pill row at all, rather than an empty strip above the input.
    let cc = field("Cc");
    cc.seed("");
    assert!(pill_labels(&cc).is_empty());
    assert!(!cc.inner.pills.is_visible());
    assert_eq!(cc.text(), "");
}

/// Typing goes into the trailing token only; the recipients already finished are carried over
/// verbatim rather than re-parsed out of the input.
pub(crate) fn typing_edits_only_the_trailing_token() {
    let to = field("To");
    to.seed("bestuur@example.test");
    to.inner.entry.set_text("gr");

    assert_eq!(to.text(), "bestuur@example.test, gr");
    assert_eq!(pill_labels(&to), vec!["bestuur@example.test"]);

    // The space survives. `current_token` trims, so a raw comparison would re-seed the input as
    // "John" the moment the space is typed: and every space after it goes the same way, so
    // "John Smith" arrives as "JohnSmith" and no name query can ever match.
    to.inner.entry.set_text("John ");
    assert_eq!(to.inner.entry.text(), "John ");
    assert_eq!(to.text(), "bestuur@example.test, John ");
}

/// Removing a pill takes out that recipient and leaves the half-typed one alone.
pub(crate) fn removing_a_pill_keeps_what_is_still_being_typed() {
    let to = field("To");
    to.seed("bestuur@example.test, tc@example.test");
    to.inner.entry.set_text("gr");

    let removes = pill_removes(&to);
    assert_eq!(removes.len(), 2, "each pill carries its own remove control");
    // It names the recipient rather than repeating a bare "Remove": three identical buttons are
    // indistinguishable to a screen reader.
    assert_eq!(
        removes[0].tooltip_text().unwrap_or_default(),
        "bestuur@example.test: Remove recipient"
    );

    removes[0].emit_clicked();
    assert_eq!(pill_labels(&to), vec!["tc@example.test"]);
    assert_eq!(to.text(), "tc@example.test, gr");
    assert_eq!(to.inner.entry.text(), "gr", "the token is untouched");
}

/// Accepting inserts the address **bare** and finishes it, so the recipients already entered
/// survive and the caret has nowhere to be but the end of an empty input.
pub(crate) fn accepting_a_suggestion_inserts_the_address_bare() {
    let to = field("To");
    to.seed("bestuur@example.test");
    to.inner.entry.set_text("gr");
    to.inner.show_suggestions(&[
        found("Greta Vos", "greta@example.test"),
        found("", "gregor@example.test"),
    ]);

    let shown = rendered_labels(to.inner.list.upcast_ref::<gtk::Widget>());
    assert!(shown.iter().any(|label| label == "Greta Vos"));
    assert!(shown.iter().any(|label| label == "greta@example.test"));
    // What a screen reader hears has no GTK getter, so it is asserted where it is observable: the
    // AT-SPI leg of `scripts/dev/test-linux-ui.sh`, which is also what caught that an explicit
    // accessible label on one of these rows is silently ignored.
    // A match known only from sent mail carries no name. It is as valid as one from a saved card,
    // usually the more useful; so it shows its address alone rather than being hidden.
    assert!(shown.iter().any(|label| label == "gregor@example.test"));
    // And one whose "name" is its own address is that same case: one line, not the address twice.
    to.inner
        .show_suggestions(&[found("bob@example.test", "bob@example.test")]);
    let repeated = rendered_labels(to.inner.list.upcast_ref::<gtk::Widget>());
    assert_eq!(
        repeated
            .iter()
            .filter(|label| *label == "bob@example.test")
            .count(),
        1,
        "an address that is also its own name is drawn once: {repeated:?}"
    );
    to.inner.entry.set_text("gr");
    to.inner.show_suggestions(&[
        found("Greta Vos", "greta@example.test"),
        found("", "gregor@example.test"),
    ]);

    let row = to
        .inner
        .list
        .row_at_index(0)
        .and_downcast::<adw::ActionRow>()
        .expect("a suggestion is a row");
    adw::prelude::ActionRowExt::activate(&row);

    assert_eq!(to.text(), "bestuur@example.test, greta@example.test, ");
    assert_eq!(
        pill_labels(&to),
        vec!["bestuur@example.test", "greta@example.test"]
    );
    assert_eq!(to.inner.entry.text(), "", "nothing is left half-typed");
}

/// Enter must accept the highlighted suggestion, which it can only do from the **capture** phase:
/// a `GtkEntry` claims Return for its own `activate` before a bubbling controller sees it, so a
/// bubble-phase handler leaves Enter moving focus while Down still highlights; a half-working
/// keyboard path that reads as "the list ignores me".
///
/// Asserted on the controller's phase rather than by delivering a key, which needs a real event
/// loop: the phase *is* the defect, and it is the part that was wrong.
pub(crate) fn the_key_handler_runs_before_the_entrys_own() {
    let to = field("To");
    let controllers = to.inner.entry.observe_controllers();
    let phases = (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .filter_map(|item| item.downcast::<gtk::EventControllerKey>().ok())
        .map(|controller| controller.propagation_phase())
        .collect::<Vec<_>>();
    assert!(
        phases.contains(&gtk::PropagationPhase::Capture),
        "the suggestion keys must be handled before the entry's own: {phases:?}"
    );
}

/// An address is the server's text, and an address book is full of ampersands. Neither a pill nor a
/// suggestion may parse one as markup: a bare `&` renders the row blank and a markup-shaped name
/// arrives styled.
pub(crate) fn a_recipients_own_text_is_never_parsed_as_markup() {
    let (_field, records) = glib_records(|| {
        let to = field("To");
        to.seed("sales&marketing@example.test");
        to.inner.entry.set_text("re");
        to.inner
            .show_suggestions(&[found("Research & Development", "r&d@example.test")]);
        to
    });

    assert!(
        !records.iter().any(|line| line.contains("from markup")),
        "a recipient's address and name must not be parsed as markup: {records:?}"
    );
}

/// The same check, on what actually reached the screen.
pub(crate) fn an_ampersand_survives_into_the_pill_and_the_suggestion() {
    let to = field("To");
    to.seed("sales&marketing@example.test");
    to.inner.entry.set_text("re");
    to.inner
        .show_suggestions(&[found("Research & Development", "r&d@example.test")]);

    assert_eq!(pill_labels(&to), vec!["sales&marketing@example.test"]);
    let shown = rendered_labels(to.inner.list.upcast_ref::<gtk::Widget>());
    assert!(
        shown.iter().any(|label| label == "Research & Development"),
        "a suggestion's name must render as itself, not blank: {shown:?}"
    );
    assert!(
        shown.iter().any(|label| label == "r&d@example.test"),
        "and so must its address: {shown:?}"
    );
}
