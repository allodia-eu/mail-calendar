use std::{cell::Cell, rc::Rc};

use adw::prelude::*;

use super::{
    GridScene, GridSurface, apply_recentre_at, rebuild_hits_with,
    scene::{DayPaint, EventPaint},
    semantic_nodes_enabled,
};
use crate::ui::{
    AppInput,
    calendar::{EventIdentity, paint::Rgb},
};

pub(crate) fn the_create_drag_owns_the_primary_pointer_before_event_buttons() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let surface = GridSurface::new(sender);
    let controllers = surface.hits.observe_controllers();
    let gesture = (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .find_map(|controller| controller.downcast::<gtk::GestureDrag>().ok())
        .expect("the calendar hit plane owns a drag controller");
    assert_eq!(gesture.propagation_phase(), gtk::PropagationPhase::Capture);
    assert_eq!(gesture.button(), gtk::gdk::BUTTON_PRIMARY);
}

pub(crate) fn recentring_releases_the_scene_before_value_notification() {
    let scene = Rc::new(std::cell::RefCell::new(GridScene::empty()));
    scene.borrow_mut().hour_height = 60.0;
    let adjustment = gtk::Adjustment::new(0.0, 0.0, 1492.0, 1.0, 60.0, 480.0);
    let notified_scene = Rc::clone(&scene);
    adjustment.connect_value_notify(move |adjustment| {
        notified_scene
            .borrow_mut()
            .set_viewport_top(adjustment.value());
    });
    let pending = Cell::new(true);

    apply_recentre_at(&adjustment, &scene, &pending, 12 * 60);

    assert!(!pending.get());
    assert!((adjustment.value() - 532.0).abs() < f64::EPSILON);
    assert!((scene.borrow().viewport_top - 532.0).abs() < f64::EPSILON);
}

/// A scene tall enough to scroll, with one event so the overlay has a hit target to destroy.
fn scrollable_scene() -> GridScene {
    let mut scene = GridScene::empty();
    scene.days = vec![DayPaint {
        date: time::Date::from_calendar_date(2026, time::Month::August, 27).unwrap(),
        label: "Thu 27".to_owned(),
        is_today: true,
    }];
    scene.events = vec![EventPaint {
        identity: EventIdentity {
            account: "alice@test.local".to_owned(),
            key: "onboarding".to_owned(),
            occurrence: "2026-08-27T13:00:00".to_owned(),
        },
        title: "Onboarding".to_owned(),
        spoken: "Onboarding, 13:00-13:30, Work".to_owned(),
        day: 0,
        start_minutes: 780,
        end_minutes: 810,
        column: 0,
        columns: 1,
        background: Rgb::new(0.0, 0.0, 0.0),
        foreground: Rgb::new(1.0, 1.0, 1.0),
        border: Rgb::new(0.0, 0.0, 0.0),
        awaiting: false,
    }];
    scene.hidden_per_day = vec![0];
    scene.hour_height = 60.0;
    scene.visible_hours = 8;
    scene.is_materialized = true;
    scene
}

/// A click on an event must not park focus on a widget that is about to be destroyed.
///
/// Opening an event rebuilds this overlay, which destroys the hit button the click landed on.
/// GTK then moves focus off the dying widget and the scrolled window *animates* to reveal
/// whatever inherits it; the first target, at the top of the day; so the grid slid away from
/// the hours the reader was looking at. Measured on the running app: 884 → 53 over five frames.
///
/// The property is the defect, so the property is what is asserted. The scroll itself cannot be
/// the oracle here: it is driven by a focus change GTK only performs for a real toplevel focus,
/// and it arrives on an easing curve some frames after the rebuild returns; so a widget test
/// that watched the adjustment would sit at its start value and pass whatever the code did.
/// `can_focus` stays on beside it: a screen reader still has to reach every event.
pub(crate) fn a_click_on_an_event_does_not_park_focus_on_the_grid() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let surface = GridSurface::new(sender.clone());
    let scene = scrollable_scene();

    // `true` rather than the desktop's own setting: with accessibility off there are no hit
    // targets at all, and a test that quietly built none would pass while the defect still ships.
    rebuild_hits_with(&surface.hits, &scene, 800.0, &sender, true);

    let mut targets = 0;
    let mut child = surface.hits.first_child();
    while let Some(widget) = child {
        assert!(
            !widget.gets_focus_on_click(),
            "a hit target that takes focus on click drags the grid away on the next render"
        );
        assert!(
            widget.can_focus(),
            "a hit target still has to be reachable from the keyboard and a screen reader"
        );
        targets += 1;
        child = widget.next_sibling();
    }
    assert!(targets > 0, "the overlay drew no hit target to assert on");
}

#[test]
fn semantic_nodes_follow_the_desktop_or_an_explicit_atspi_session() {
    assert!(semantic_nodes_enabled(true, None));
    assert!(semantic_nodes_enabled(false, Some("atspi")));
    assert!(!semantic_nodes_enabled(false, None));
}
