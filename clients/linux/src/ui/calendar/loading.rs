//! The pill a calendar surface lays over its grid while the period is loading.
//!
//! A product decision, the same on every client (`docs/calendar.md`, "Showing a period that is
//! loading"): a period the core has not materialized keeps its grid, and a spinner beside "loading
//! this period" floats over it, pinned to what is on screen rather than to the scrolled day. The
//! words alone, drawn at the top of a day that opens scrolled to the morning, sat above the
//! viewport on an otherwise blank surface.

use adw::prelude::*;

use crate::l10n;

/// The pill, shown while its surface's period is loading.
pub(super) struct LoadingPill {
    pill: gtk::Box,
}

impl LoadingPill {
    /// Lays a pill over `content`, returning the overlay that now holds both, and the pill.
    ///
    /// `spoken` is whether the pill is the surface's spoken status. The week grid's semantic
    /// overlay already says the period is loading, so its pill stays silent rather than say it
    /// twice; the month has no such node, so its pill is what a screen reader hears.
    pub(super) fn over(content: &impl IsA<gtk::Widget>, spoken: bool) -> (gtk::Overlay, Self) {
        install_css();
        let pill = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        pill.add_css_class("osd");
        pill.add_css_class("calendar-loading-pill");
        pill.set_halign(gtk::Align::Center);
        pill.set_valign(gtk::Align::Start);
        pill.set_margin_top(16);
        // The grid under it stays usable: a press goes through to the day it lands on.
        pill.set_can_target(false);
        pill.append(&adw::Spinner::new());
        pill.append(&gtk::Label::new(Some(l10n::calendar_loading_range())));
        if !spoken {
            pill.update_state(&[gtk::accessible::State::Hidden(true)]);
        }
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(content));
        overlay.add_overlay(&pill);
        (overlay, Self { pill })
    }

    /// Shows the pill while `loading`, and hides it once the period is materialized.
    pub(super) fn show(&self, loading: bool) {
        self.pill.set_visible(loading);
    }

    #[cfg(test)]
    pub(super) fn is_shown(&self) -> bool {
        self.pill.is_visible()
    }

    #[cfg(test)]
    pub(super) fn widget(&self) -> &gtk::Box {
        &self.pill
    }
}

fn install_css() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            ".calendar-loading-pill { border-radius: 999px; padding: 6px 14px; }",
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
