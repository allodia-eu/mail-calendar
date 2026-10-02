//! Pointer hits and the AT-SPI overlay, for each of the grid's two surfaces.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;

use super::{
    super::super::AppInput,
    geometry::Hit,
    scene::{GUTTER, GridScene},
};

/// Which of the grid's surfaces a hit plane lies over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Surface {
    /// The pinned header: all-day bars and the overflow chips.
    Header,
    /// The scrolled hours: timed events.
    Hours,
}

impl Surface {
    fn hits(self, scene: &GridScene, width: f64) -> Vec<Hit> {
        let geometry = scene.geometry(width);
        match self {
            Self::Header => geometry.header_hits,
            Self::Hours => geometry.hits,
        }
    }
}

/// Opens the event under a click, or expands the banner from its overflow chip.
///
/// `hours` is the hours surface, whose width both surfaces' columns are measured against.
pub(super) fn install_event_click(
    plane: &gtk::Fixed,
    hours: &gtk::DrawingArea,
    scene: &std::rc::Rc<std::cell::RefCell<GridScene>>,
    sender: &relm4::Sender<AppInput>,
    surface: Surface,
) {
    let gesture = gtk::GestureClick::new();
    gesture.set_button(gtk::gdk::BUTTON_PRIMARY);
    let click_scene = std::rc::Rc::clone(scene);
    let click_hours = hours.clone();
    let input = sender.clone();
    gesture.connect_released(move |_, presses, x, y| {
        if presses != 1 || semantic_nodes_active() {
            return;
        }
        let hit = surface
            .hits(&click_scene.borrow(), f64::from(click_hours.width()))
            .into_iter()
            .find(|hit| hit.contains(x, y));
        if let Some(hit) = hit {
            if let Some(identity) = hit.identity {
                input.emit(AppInput::OpenCalendarEvent(identity));
            } else {
                input.emit(AppInput::ToggleAllDay);
            }
        }
    });
    plane.add_controller(gesture);
}

pub(super) fn rebuild_hits(
    fixed: &gtk::Fixed,
    scene: &GridScene,
    width: f64,
    sender: &relm4::Sender<AppInput>,
    surface: Surface,
) {
    rebuild_hits_with(
        fixed,
        scene,
        width,
        sender,
        surface,
        semantic_nodes_active(),
    );
}

pub(super) fn rebuild_hits_with(
    fixed: &gtk::Fixed,
    scene: &GridScene,
    width: f64,
    sender: &relm4::Sender<AppInput>,
    surface: Surface,
    semantic: bool,
) {
    while let Some(child) = fixed.first_child() {
        fixed.remove(&child);
    }
    if !semantic {
        return;
    }
    if let Some(status) = scene.semantic_status() {
        if surface == Surface::Hours {
            append_loading_node(fixed, width, &status);
        }
        return;
    }
    for hit in surface.hits(scene, width) {
        let button = gtk::Button::new();
        button.add_css_class("calendar-hit-target");
        // Keyboard-reachable, but never focused *by the click itself*. Opening an event rebuilds
        // this overlay, which destroys the button the click landed on; GTK then moves focus off
        // the dying widget and the scrolled window animates to reveal whatever inherits it,
        // the first target, at the top of the day. The reader was hours further down.
        button.set_focus_on_click(false);
        button.set_tooltip_text(Some(&hit.spoken));
        button.update_property(&[AccessibleProperty::Label(&hit.spoken)]);
        button.set_size_request(
            pixel_size(hit.rect.width.max(1.0).round()),
            pixel_size(hit.rect.height.max(1.0).round()),
        );
        let input = sender.clone();
        if let Some(identity) = hit.identity {
            button.connect_clicked(move |_| {
                input.emit(AppInput::OpenCalendarEvent(identity.clone()));
            });
        } else {
            button.connect_clicked(move |_| input.emit(AppInput::ToggleAllDay));
        }
        fixed.put(&button, hit.rect.x, hit.rect.y);
    }
}

pub(super) fn semantic_nodes_active() -> bool {
    semantic_nodes_enabled(
        gtk::gio::Settings::new("org.gnome.desktop.interface").boolean("toolkit-accessibility"),
        std::env::var("GTK_A11Y").ok().as_deref(),
    )
}

pub(super) fn semantic_nodes_enabled(
    toolkit_accessibility: bool,
    requested_backend: Option<&str>,
) -> bool {
    toolkit_accessibility
        || requested_backend.is_some_and(|value| value.eq_ignore_ascii_case("atspi"))
}

fn append_loading_node(fixed: &gtk::Fixed, width: f64, status: &str) {
    let loading = gtk::Label::new(Some(status));
    loading.add_css_class("calendar-loading-target");
    loading.update_property(&[AccessibleProperty::Label(status)]);
    loading.set_size_request(pixel_size((width - GUTTER).max(1.0)), 48);
    fixed.put(&loading, GUTTER, 0.0);
}

/// Calendar geometry is bounded to a few thousand pixels; clamp before the GTK integer boundary.
#[allow(clippy::cast_possible_truncation)]
pub(super) fn pixel_size(value: f64) -> i32 {
    value.clamp(1.0, f64::from(i32::MAX)).round() as i32
}

pub(super) fn install_hit_target_css() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            ".calendar-hit-target { background: transparent; color: transparent; border-color: transparent; box-shadow: none; padding: 0; min-width: 0; min-height: 0; } .calendar-loading-target { color: transparent; }",
        );
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}
