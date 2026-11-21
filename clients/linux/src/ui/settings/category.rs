//! What each Settings category is called (its stack name, its title and its glyph), the sidebar
//! row that opens it, and which one the window opens on.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;

use super::{Category, icons};
use crate::l10n;

/// The sidebar's rows as the window drew them, so a page can send the person to another
/// category, as a remedy that lives elsewhere in Settings does.
#[derive(Clone, Default)]
pub(super) struct Sidebar {
    rows: Rc<RefCell<Vec<(Category, gtk::ToggleButton)>>>,
    /// Where a category sent to takes the focus, so the page scrolls to what sent the person
    /// there: the own endpoint's address, at the foot of Advanced.
    anchors: Rc<RefCell<Vec<Anchor>>>,
}

type Anchor = (Category, gtk::glib::WeakRef<gtk::Widget>);

impl Sidebar {
    pub(super) fn register(&self, category: Category, button: &gtk::ToggleButton) {
        self.rows.borrow_mut().push((category, button.clone()));
    }

    pub(super) fn anchor(&self, category: Category, widget: &gtk::Widget) {
        self.anchors
            .borrow_mut()
            .push((category, widget.downgrade()));
    }

    /// Shows `category`, when the window has it, with the focus on its anchor.
    pub(super) fn show(&self, category: Category) {
        let button = self
            .rows
            .borrow()
            .iter()
            .find(|(shown, _)| *shown == category)
            .map(|(_, button)| button.clone());
        if let Some(button) = button {
            button.set_active(true);
        }
        let anchor = self
            .anchors
            .borrow()
            .iter()
            .find(|(anchored, _)| *anchored == category)
            .and_then(|(_, widget)| widget.upgrade());
        if let Some(anchor) = anchor {
            // Once the stack has the page on screen, which takes it a turn of the loop.
            gtk::glib::idle_add_local_once(move || reveal(&anchor));
        }
    }
}

impl Category {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Allodia => "allodia",
            Self::General => "general",
            Self::Calendar => "calendar",
            Self::Videoconferencing => "videoconferencing",
            Self::Reading => "reading",
            Self::Composing => "composing",
            Self::Signatures => "signatures",
            Self::WritingStyle => "writing-style",
            Self::Notifications => "notifications",
            Self::Privacy => "privacy",
            Self::Accounts => "accounts",
            Self::Advanced => "advanced",
            Self::Diagnostics => "diagnostics",
            Self::About => "about",
        }
    }

    pub(super) fn title(self) -> &'static str {
        match self {
            // The category and the group inside it are the same thing named twice otherwise, and
            // the product's own name belongs in one string.
            Self::Allodia => l10n::settings_allodia_heading(),
            Self::General => l10n::settings_category_general(),
            Self::Calendar => l10n::settings_category_calendar(),
            Self::Videoconferencing => l10n::settings_category_videoconferencing(),
            Self::Reading => l10n::settings_category_reading(),
            Self::Composing => l10n::settings_category_composing(),
            Self::Signatures => l10n::settings_category_signatures(),
            Self::WritingStyle => l10n::settings_category_writing_style(),
            Self::Notifications => l10n::settings_category_notifications(),
            Self::Privacy => l10n::settings_category_privacy(),
            Self::Accounts => l10n::settings_category_accounts(),
            Self::Advanced => l10n::settings_category_advanced(),
            Self::Diagnostics => l10n::settings_category_diagnostics(),
            Self::About => l10n::settings_category_about(),
        }
    }

    pub(super) const fn icon(self) -> &'static str {
        match self {
            Self::Allodia => icons::SETTINGS_ALLODIA,
            Self::General => icons::SETTINGS_GENERAL,
            Self::Calendar => icons::SETTINGS_CALENDAR,
            Self::Videoconferencing => icons::SETTINGS_VIDEOCONFERENCING,
            Self::Reading => icons::SETTINGS_READING,
            Self::Composing => icons::SETTINGS_COMPOSING,
            Self::Signatures => icons::SETTINGS_SIGNATURES,
            Self::WritingStyle => icons::SETTINGS_WRITING_STYLE,
            Self::Notifications => icons::SETTINGS_NOTIFICATIONS,
            Self::Privacy => icons::SETTINGS_PRIVACY,
            Self::Accounts => icons::SETTINGS_ACCOUNTS,
            Self::Advanced => icons::SETTINGS_ADVANCED,
            Self::Diagnostics => icons::SETTINGS_DIAGNOSTICS,
            Self::About => icons::SETTINGS_ABOUT,
        }
    }
}

/// Scrolls the page holding `anchor` so it starts at the top, and puts the focus in its first
/// field.
fn reveal(anchor: &gtk::Widget) {
    let scroll = std::iter::successors(anchor.parent(), gtk::Widget::parent)
        .find_map(|widget| widget.downcast::<gtk::ScrolledWindow>().ok());
    if let Some(scroll) = scroll
        && let Some(content) = scroll.child()
        && let Some(point) = anchor.compute_point(&content, &gtk::graphene::Point::new(0.0, 0.0))
    {
        // The point is where the anchor is drawn now, from the top of what is in view.
        let adjustment = scroll.vadjustment();
        adjustment.set_value(adjustment.value() + f64::from(point.y()) - 12.0);
    }
    anchor.child_focus(gtk::DirectionType::TabForward);
}

/// One sidebar row's button: the category's glyph and its name, the way every other platform's
/// settings sidebar draws it (docs/settings.md).
///
/// The name is set on the button as well as drawn inside it, so assistive technology and the UI
/// suite find the row by the same words whatever the content widget does with them.
pub(super) fn category_button(category: Category) -> gtk::ToggleButton {
    let content = adw::ButtonContent::new();
    content.set_icon_name(category.icon());
    content.set_label(category.title());
    content.set_halign(gtk::Align::Start);
    let button = gtk::ToggleButton::new();
    button.set_child(Some(&content));
    button.add_css_class("flat");
    button.set_halign(gtk::Align::Fill);
    button.set_hexpand(true);
    button.update_property(&[gtk::accessible::Property::Label(category.title())]);
    button
}

/// Which category the window opens on: the one the caller asked for, except when the screenshot
/// hook names `signatures`.
///
/// The hook wins over the request so a capture filed under `signatures` cannot silently show a
/// different category. Normal navigation uses the accessible toggle buttons in the sidebar.
///
/// `shown` is the sidebar's own list, never every category: counted over every category instead, a
/// build carrying no registration selects the row *after* the one asked for, and the last category
/// selects no row at all.
pub(super) fn initial_category(shown: &[Category], requested: Category) -> i32 {
    #[cfg(any(debug_assertions, feature = "dev-harness"))]
    if crate::showcase::screen() == Ok(crate::showcase::ShowcaseScreen::Signatures)
        && let Some(index) = index_of(shown, Category::Signatures)
    {
        return index;
    }
    index_of(shown, requested).unwrap_or_default()
}

fn index_of(shown: &[Category], wanted: Category) -> Option<i32> {
    shown
        .iter()
        .position(|category| *category == wanted)
        .and_then(|index| i32::try_from(index).ok())
}
