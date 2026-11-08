//! The grid's labels against the real text stack: wrapped by word and never inside one, and laid
//! out once per page however often the page is drawn (`docs/calendar.md` §4 and §7).

use adw::prelude::*;

use super::{
    GridSurface,
    labels::Label,
    scene::{DayPaint, EventPaint, GridScene},
};
use crate::ui::{
    AppInput,
    calendar::{
        EventIdentity,
        blocks::{self, LABEL_STEP},
        paint::{Rgb, event_colors},
    },
};

pub(crate) fn a_title_wraps_by_word_and_a_word_wider_than_the_block_is_one_line() {
    let widget = gtk::Label::new(None);
    let context = widget.pango_context();
    let black = Rgb::new(0.0, 0.0, 0.0);

    let mut label = Label::new(&context, "Quarterly planning review", 11.0, black);
    let line = label.line;
    let wrap = blocks::wrap_width(label.widest + LABEL_STEP);
    assert!(blocks::wraps(label.widest, wrap));
    let text = label.wrapped(wrap);
    // More than one line, none wider than the room it was wrapped to: Pango breaks a word only
    // when the word alone is wider than the line, which `wraps` rules out.
    let lines = text.line_bottoms.len();
    assert!(lines >= 2, "{lines} lines");
    assert!(text.width <= wrap, "{} wider than {wrap}", text.width);
    // Cut after a whole line, at the first line's own height, and never taller than the text.
    assert!((text.height_of(1) - line).abs() < 1.0);
    assert!(text.height_of(1) < text.height_of(2));
    assert!((text.height_of(99) - text.height).abs() < 1.0);

    let narrow = Label::new(&context, "Onboarding", 11.0, black);
    assert!(!blocks::wraps(
        narrow.widest,
        blocks::wrap_width(narrow.widest - 1.0)
    ));
}

pub(crate) fn a_label_is_laid_out_once_per_page_however_often_it_is_drawn() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let surface = GridSurface::new(sender);
    let mut scene = GridScene::empty();
    scene.days = vec![DayPaint {
        date: time::Date::from_calendar_date(2026, time::Month::August, 27).unwrap(),
        label: "Thu 27".to_owned(),
        is_today: false,
    }];
    scene.events = vec![
        event("standup", "Daily standup", 0, 9 * 60, false),
        event("review", "Quarterly planning review", 0, 10 * 60, true),
    ];
    scene.hidden_per_day = vec![0];
    scene.visible_hours = 24;
    scene.is_materialized = true;
    scene.timezone = "UTC".to_owned();
    *surface.scene.borrow_mut() = scene;
    let window = gtk::Window::new();
    window.set_default_size(600, 1600);
    window.set_child(Some(&surface.root));
    window.present();
    settle(|| surface.labels.borrow().len() == 2);
    assert_eq!(surface.labels.borrow().len(), 2);

    for _ in 0..3 {
        surface.drawing.queue_draw();
        settle(|| false);
    }
    assert_eq!(surface.labels.borrow().len(), 2);
    window.destroy();
}

fn event(key: &str, title: &str, day: usize, start: u32, awaiting: bool) -> EventPaint {
    EventPaint {
        identity: EventIdentity {
            account: "alice@test.local".to_owned(),
            key: key.to_owned(),
            occurrence: String::new(),
        },
        title: title.to_owned(),
        spoken: title.to_owned(),
        has_conference: true,
        day,
        start_minutes: start,
        end_minutes: start + 60,
        column: 0,
        columns: 1,
        colors: event_colors("#2f6fa8", false),
        awaiting,
        repeats: true,
        time: "09:00".to_owned(),
        range: "09:00–10:00".to_owned(),
    }
}

fn settle(ready: impl Fn() -> bool) {
    let context = gtk::glib::MainContext::default();
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while std::time::Instant::now() < deadline {
        while context.iteration(false) {}
        if ready() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
