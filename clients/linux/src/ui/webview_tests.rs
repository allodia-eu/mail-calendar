//! What the reading host does between one document and the next: when the page it is showing is
//! actually on screen, and what the reader's zoom is worth once they open something else.
//!
//! The GTK half is called from the crate's single `gtk::init` test (see
//! [`super::super::mailbox::tests`]); the paint gate is a plain state machine and needs no display.

use gtk::{glib, prelude::*};
use webkit6::{LoadEvent, prelude::*};

use super::{DocumentKind, PaintGate, SecureWebView, clamp_zoom};

/// The sequences below are what WebKitGTK actually emits, read off a `load-changed` handler
/// on this toolkit version. They are fixtures rather than a reading of the documentation,
/// because the documented order is the one this gate already got wrong.
///
/// The reading pane reveals the view on `painted()`, so a gate that answers `true` too early
/// puts the black first frame back, and one that never answers `true` leaves the body area
/// blank for the life of the message. Only the first is recoverable by the next render, which
/// is why `Started` gates `Finished` rather than a count of loads in flight: a spurious
/// `Finished` costs one early reveal, a missing one costs the message.
#[test]
fn a_cancelled_load_does_not_hand_its_completion_to_the_next_message() {
    // Two opens with a render turn between them, which is every open: the first load has
    // committed by the time the second is asked for, and its `Finished` then arrives *after*
    // the call that cancelled it and *before* the new load starts.
    let mut gate = PaintGate::default();
    gate.asked();
    assert!(!gate.observed(LoadEvent::Started));
    assert!(!gate.observed(LoadEvent::Committed));
    assert!(gate.observed(LoadEvent::Finished), "the first page arrives");
    assert!(gate.painted());

    gate.asked();
    assert!(
        !gate.painted(),
        "nothing on screen belongs to the new message"
    );
    assert!(
        !gate.observed(LoadEvent::Finished),
        "the cancelled load's completion is not this message's"
    );
    assert!(
        !gate.painted(),
        "revealing here shows a view that has not painted, which is the black frame"
    );
    assert!(!gate.observed(LoadEvent::Started));
    assert!(!gate.observed(LoadEvent::Committed));
    assert!(gate.observed(LoadEvent::Finished));
    assert!(gate.painted());
}

#[test]
fn two_asks_inside_one_turn_still_reveal_the_page() {
    // `load_html` twice with no turn between coalesces: the first load never starts, so only
    // one set of events ever arrives. A gate counting loads in flight would wait for a second
    // `Finished` that is never coming, and hold the body area blank.
    let mut gate = PaintGate::default();
    gate.asked();
    gate.asked();
    assert!(!gate.observed(LoadEvent::Started));
    assert!(!gate.observed(LoadEvent::Committed));
    assert!(gate.observed(LoadEvent::Finished));
    assert!(gate.painted());
}

#[test]
fn a_pinch_cannot_leave_the_message_at_a_size_with_no_way_back() {
    // A pinch reports an unbounded scale, so the level it is multiplied into has to be held
    // inside the range the reader can gesture their way out of (`docs/reading-zoom.md`).
    assert!((clamp_zoom(1.0) - 1.0).abs() < f64::EPSILON);
    assert!((clamp_zoom(0.01) - 0.25).abs() < f64::EPSILON);
    assert!((clamp_zoom(400.0) - 5.0).abs() < f64::EPSILON);
}

/// Both zoom controllers listen in the **capture** phase, and only the reading host has them.
///
/// The phase is the half that fails silently: the web view claims a pinch and a scroll for its own
/// handling, so a controller left in GTK's default `Bubble` phase is never reached and the gesture
/// does nothing at all, on a build that compiles and a screenshot that looks right.
pub(crate) fn the_readers_zoom_gestures_listen_ahead_of_the_web_view() {
    let (sender, _receiver) = relm4::channel::<super::AppInput>();
    let reading = SecureWebView::new(DocumentKind::Reading, sender.clone());

    let controllers = reading.widget().observe_controllers();
    let installed: Vec<glib::Object> = (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .collect();

    let pinch = installed
        .iter()
        .find_map(|item| item.clone().downcast::<gtk::GestureZoom>().ok())
        .expect("the reader can pinch a message");
    assert_eq!(pinch.propagation_phase(), gtk::PropagationPhase::Capture);

    let scroll = installed
        .iter()
        .find_map(|item| item.clone().downcast::<gtk::EventControllerScroll>().ok())
        .expect("the reader can Ctrl+scroll a message");
    assert_eq!(scroll.propagation_phase(), gtk::PropagationPhase::Capture);

    // The composer is a document the user is writing, not one they are reading, and its own
    // editing gestures are the web view's: it gets neither controller.
    let composer = SecureWebView::new(DocumentKind::Composer, sender);
    let composer_controllers = composer.widget().observe_controllers();
    assert!(
        (0..composer_controllers.n_items())
            .filter_map(|index| composer_controllers.item(index))
            .all(|item| item.downcast::<gtk::GestureZoom>().is_err()),
        "only the reading host zooms"
    );
}

/// A zoom belongs to the message it was made on (`docs/reading-zoom.md`).
///
/// `zoom-level` is the *view's*, not the document's, so it survives `load_html` and would otherwise
/// hand the next message a scale chosen for the last one. Asserted on the widget rather than on the
/// reset call, because the two are one line apart and the bug is that the line is missing.
pub(crate) fn opening_another_message_starts_again_at_its_own_fit() {
    let (sender, _receiver) = relm4::channel::<super::AppInput>();
    let web = SecureWebView::new(DocumentKind::Reading, sender);
    let document = "<!doctype html><html><body>a message</body></html>";

    web.load(document, false);
    web.widget().set_zoom_level(2.5);

    // A different document: the same one is deduplicated by `load` and must not reset anything,
    // since that is one message being re-rendered (the remote-images opt-in) rather than another
    // being opened.
    web.load(
        "<!doctype html><html><body>another message</body></html>",
        false,
    );
    assert!(
        (web.widget().zoom_level() - 1.0).abs() < f64::EPSILON,
        "the next message opens at its own fit, not the last one's zoom"
    );

    web.widget().set_zoom_level(2.5);
    web.clear();
    assert!(
        (web.widget().zoom_level() - 1.0).abs() < f64::EPSILON,
        "an emptied pane keeps no zoom for whatever is opened next"
    );
}
