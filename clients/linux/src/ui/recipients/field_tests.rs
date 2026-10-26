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
    assert_eq!(pill_labels(&cc), [] as [String; 0]);
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

/// The field asks an on-screen keyboard for its email layout, as every address field does.
pub(crate) fn the_field_asks_for_the_email_keyboard() {
    let to = field("To");

    assert_eq!(to.inner.entry.input_purpose(), gtk::InputPurpose::Email);
    let hints = to.inner.entry.input_hints();
    assert!(
        hints.contains(gtk::InputHints::NO_SPELLCHECK),
        "an address is not a word"
    );
    assert!(
        hints.contains(gtk::InputHints::LOWERCASE),
        "an address is not a sentence"
    );
}

/// Ctrl+Z after finishing an address takes the address back to text being typed, and a second
/// one takes the typing back too. Left to the entry, its own history typed the finished address
/// back in and the field finished it again, so the recipient came back twice.
pub(crate) fn undo_takes_back_a_finished_address_without_doubling_it() {
    let to = field("To");
    // A keystroke inserts; `set_text` would empty the entry first and pass through "".
    let mut position = 0;
    to.inner
        .entry
        .insert_text("bob@example.test", &mut position);
    to.inner.entry.insert_text(",", &mut position);
    assert_eq!(pill_labels(&to), vec!["bob@example.test"]);
    assert!(
        !to.inner.entry.enables_undo(),
        "the entry's own history must stay out of it"
    );

    let ctrl = gtk::gdk::ModifierType::CONTROL_MASK;
    to.inner.key_pressed(gtk::gdk::Key::z, 0, ctrl);
    assert!(pill_labels(&to).is_empty(), "{:?}", pill_labels(&to));
    assert_eq!(to.inner.entry.text(), "bob@example.test");
    to.inner.key_pressed(gtk::gdk::Key::z, 0, ctrl);
    assert_eq!(to.text(), "");

    let shift = gtk::gdk::ModifierType::SHIFT_MASK;
    to.inner.key_pressed(gtk::gdk::Key::Z, 0, ctrl | shift);
    to.inner.key_pressed(gtk::gdk::Key::y, 0, ctrl);
    assert_eq!(pill_labels(&to), vec!["bob@example.test"]);
}

/// A recipient removed by its pill's button comes back with Ctrl+Z, where it was.
pub(crate) fn undo_brings_back_a_removed_recipient() {
    let to = field("To");
    to.seed("bestuur@example.test, tc@example.test");
    pill_removes(&to)[0].emit_clicked();
    assert_eq!(pill_labels(&to), vec!["tc@example.test"]);

    to.inner
        .key_pressed(gtk::gdk::Key::z, 0, gtk::gdk::ModifierType::CONTROL_MASK);
    assert_eq!(
        pill_labels(&to),
        vec!["bestuur@example.test", "tc@example.test"]
    );
    // The seed is where the history starts: the field was opened holding both.
    to.inner
        .key_pressed(gtk::gdk::Key::z, 0, gtk::gdk::ModifierType::CONTROL_MASK);
    assert_eq!(
        pill_labels(&to),
        vec!["bestuur@example.test", "tc@example.test"]
    );
}

/// On a layout with no Latin letter on the Z key, Ctrl+Z is still undo: the chord is read off the
/// first layout's letter on the same key, as GTK's own bindings read it. The entry's own undo is
/// off, so without this the field had no undo at all there.
pub(crate) fn the_undo_chord_is_found_on_a_layout_without_latin_letters() {
    let to = field("To");
    let display = to.inner.entry.display();
    let keys = display
        .map_keyval(gtk::gdk::Key::z)
        .expect("the test display has a keymap");
    let z = keys
        .iter()
        .find(|key| key.group() == 0 && key.level() == 0)
        .expect("Z is on the first layout");
    assert_eq!(
        super::super::history::chord_key(&display, gtk::gdk::Key::Cyrillic_ya, z.keycode()),
        gtk::gdk::Key::z
    );
    assert_eq!(
        super::super::history::chord_key(&display, gtk::gdk::Key::w, z.keycode()),
        gtk::gdk::Key::w,
        "a Latin layout's own letter wins over the key it is on"
    );
}

/// Enter finishes the address being typed as a comma would, and in an empty field it is left to
/// the entry's own `activate` (docs/contacts.md).
pub(crate) fn enter_finishes_the_typed_address() {
    let to = field("To");
    let mut position = 0;
    to.inner
        .entry
        .insert_text("bob@example.test", &mut position);
    let none = gtk::gdk::ModifierType::empty();

    let taken = to.inner.key_pressed(gtk::gdk::Key::Return, 0, none);
    assert_eq!(taken, gtk::glib::Propagation::Stop);
    assert_eq!(pill_labels(&to), vec!["bob@example.test"]);
    assert_eq!(to.inner.entry.text(), "");

    let left = to.inner.key_pressed(gtk::gdk::Key::Return, 0, none);
    assert_eq!(
        left,
        gtk::glib::Propagation::Proceed,
        "nothing typed, nothing taken"
    );

    // One step: Ctrl+Z gives the address back as text.
    to.inner
        .key_pressed(gtk::gdk::Key::z, 0, gtk::gdk::ModifierType::CONTROL_MASK);
    assert_eq!(to.inner.entry.text(), "bob@example.test");
}

/// Enter in a field whose owner takes it (the invitee list) is the owner's, and finishes nothing.
pub(crate) fn an_owners_enter_finishes_nothing() {
    let invitees = field("Invitees");
    invitees.connect_activate(|| {});
    let mut position = 0;
    invitees
        .inner
        .entry
        .insert_text("bob@example.test", &mut position);
    let enter =
        invitees
            .inner
            .key_pressed(gtk::gdk::Key::Return, 0, gtk::gdk::ModifierType::empty());
    assert_eq!(enter, gtk::glib::Propagation::Proceed);
    assert_eq!(pill_labels(&invitees), [] as [String; 0]);
}

/// The field is left when focus lands outside it, and only then: a pill's remove button is still
/// the field, and a window gone inactive is the user leaving the app (docs/contacts.md).
pub(crate) fn only_focus_outside_the_field_leaves_it() {
    let to = field("To");
    to.seed("bestuur@example.test");
    let root = to.inner.root.upcast_ref::<gtk::Widget>();
    let remove = pill_removes(&to).remove(0);
    let entry = to.inner.entry.upcast_ref::<gtk::Widget>();
    let subject = gtk::Entry::new();
    let subject = subject.upcast_ref::<gtk::Widget>();

    assert!(super::focus_left(root, true, Some(subject)));
    assert!(super::focus_left(root, true, None));
    assert!(!super::focus_left(root, true, Some(entry)));
    assert!(!super::focus_left(root, true, Some(remove.upcast_ref())));
    assert!(!super::focus_left(root, false, Some(subject)));

    let controllers = to.inner.root.observe_controllers();
    let watched = (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .any(|item| item.is::<gtk::EventControllerFocus>());
    assert!(
        watched,
        "the whole field watches its focus, not only the entry"
    );
}
