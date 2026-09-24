//! The frame the two Writing style sheets share (`docs/ai.md`, "Learning"): a Settings detail whose
//! pages sit side by side in a carousel, with the page dots between Back and Next.
//!
//! Only the buttons move the pages, so a scroll inside a page never turns one. The move is the
//! carousel's own animation, which libadwaita skips when the desktop turns animations off, the
//! GNOME setting behind "Reduce animation".

use adw::prelude::*;
use gtk::accessible::{Property, State};

use super::dialog_box;
use crate::{l10n, ui::writing_style_reveal::WizardPager};

pub(super) struct Wizard {
    pub(super) root: adw::ToolbarView,
    pub(super) header: adw::HeaderBar,
    pub(super) cancel: gtk::Button,
    pub(super) back: gtk::Button,
    /// Next, or what stands in its place: Save, Learn, Stop, Close.
    pub(super) primary: gtk::Button,
    carousel: adw::Carousel,
    dots: adw::CarouselIndicatorDots,
}

impl Wizard {
    pub(super) fn new(title: &str) -> Self {
        let root = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new(title, "")));
        // The Settings header above already closes the window. A second one here would sit
        // beside a Close that closes only the sheet, under the same name.
        header.set_show_end_title_buttons(false);
        let cancel = gtk::Button::with_label(l10n::action_cancel());
        header.pack_start(&cancel);
        root.add_top_bar(&header);

        let carousel = adw::Carousel::new();
        carousel.set_interactive(false);
        carousel.set_allow_scroll_wheel(false);
        carousel.set_hexpand(true);
        carousel.set_vexpand(true);
        root.set_content(Some(&carousel));

        // An image to assistive technology, named by the step it shows: the dots themselves say
        // nothing a screen reader can read.
        let dots = adw::CarouselIndicatorDots::builder()
            .accessible_role(gtk::AccessibleRole::Img)
            .carousel(&carousel)
            .build();
        let back = gtk::Button::with_label(l10n::wizard_back());
        let primary = gtk::Button::with_label(l10n::wizard_next());
        primary.add_css_class("suggested-action");
        // A centre box keeps the dots centred and Next where it is while Back comes and goes.
        let footer = gtk::CenterBox::new();
        footer.set_margin_top(10);
        footer.set_margin_bottom(10);
        footer.set_margin_start(16);
        footer.set_margin_end(16);
        footer.set_start_widget(Some(&back));
        footer.set_center_widget(Some(&dots));
        footer.set_end_widget(Some(&primary));
        root.add_bottom_bar(&footer);
        root.set_bottom_bar_style(adw::ToolbarStyle::Raised);
        Self {
            root,
            header,
            cancel,
            back,
            primary,
            carousel,
            dots,
        }
    }

    pub(super) fn append(&self, page: &WizardPage) {
        self.carousel.append(&page.root);
    }

    /// Turns to the page `pager` stands on. The pages beside it stay laid out in the carousel, so
    /// they are kept from the keyboard and from assistive technology.
    pub(super) fn show(&self, pager: WizardPager) {
        let index = u32::try_from(pager.index()).unwrap_or(u32::MAX);
        for position in 0..self.carousel.n_pages() {
            let page = self.carousel.nth_page(position);
            let current = position == index;
            page.set_can_focus(current);
            page.update_state(&[State::Hidden(!current)]);
            if current {
                self.carousel.scroll_to(&page, true);
            }
        }
        // Through `Widget`: the binding lists `Accessible` on the dots only under its `gtk_v4_10`
        // feature, which this crate does not turn on.
        self.dots
            .upcast_ref::<gtk::Widget>()
            .update_property(&[Property::Label(&pager.step_label())]);
        self.back.set_visible(!pager.is_first());
    }

    /// Puts `label` where Next stands: the prominent look for a step forward, the plain one for
    /// Stop.
    pub(super) fn set_primary(&self, label: &str, prominent: bool, enabled: bool) {
        self.primary.set_label(label);
        if prominent {
            self.primary.add_css_class("suggested-action");
        } else {
            self.primary.remove_css_class("suggested-action");
        }
        self.primary.set_sensitive(enabled);
        self.primary.set_visible(true);
    }
}

/// One page: its heading over a body a sheet fills and, for some pages, redraws.
pub(super) struct WizardPage {
    root: gtk::ScrolledWindow,
    pub(super) title: gtk::Label,
    pub(super) body: gtk::Box,
}

impl WizardPage {
    pub(super) fn new(title: &str) -> Self {
        let content = dialog_box();
        let heading = gtk::Label::new(Some(title));
        heading.add_css_class("title-2");
        heading.set_wrap(true);
        heading.set_xalign(0.0);
        content.append(&heading);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.append(&body);
        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(640);
        clamp.set_child(Some(&content));
        let root = gtk::ScrolledWindow::new();
        root.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.set_child(Some(&clamp));
        Self {
            root,
            title: heading,
            body,
        }
    }

    /// Empties the body for a redraw.
    pub(super) fn clear(&self) {
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
    }
}
