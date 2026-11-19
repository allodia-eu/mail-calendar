//! Pages a flow step shows one at a time (`docs/ai.md`, "The flow host"): an `AdwCarousel` with
//! its `AdwCarouselIndicatorDots`, moved only by the buttons, so a scroll inside a page never turns
//! one. It keeps which page is on screen and nothing else. The move is the carousel's own
//! animation, which libadwaita skips when the desktop turns animations off.

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use gtk::accessible::State;

use crate::{l10n, ui::icons};

pub(crate) struct FlowPager {
    carousel: adw::Carousel,
    footer: gtk::CenterBox,
    back: gtk::Button,
    index: Cell<u32>,
}

impl FlowPager {
    /// A pager over `shown`, on its first page.
    pub(crate) fn new(shown: &[gtk::Widget]) -> Rc<Self> {
        let carousel = adw::Carousel::new();
        carousel.set_interactive(false);
        carousel.set_allow_scroll_wheel(false);
        carousel.set_allow_mouse_drag(false);
        carousel.set_hexpand(true);
        carousel.set_vexpand(true);
        for page in shown {
            carousel.append(page);
        }
        // The dots say nothing a screen reader can read; the page's own heading does.
        let dots = adw::CarouselIndicatorDots::builder()
            .carousel(&carousel)
            .accessible_role(gtk::AccessibleRole::Presentation)
            .build();
        let footer = gtk::CenterBox::new();
        footer.set_margin_top(6);
        footer.set_margin_bottom(6);
        footer.set_center_widget(Some(&dots));
        let back = gtk::Button::from_icon_name(icons::PREVIOUS);
        back.set_tooltip_text(Some(l10n::wizard_back()));
        back.update_property(&[gtk::accessible::Property::Label(l10n::wizard_back())]);
        let pager = Rc::new(Self {
            carousel,
            footer,
            back,
            index: Cell::new(0),
        });
        let weak = Rc::downgrade(&pager);
        pager.back.connect_clicked(move |_| {
            if let Some(pager) = weak.upgrade() {
                pager.turn(false);
            }
        });
        pager.show();
        pager
    }

    pub(crate) fn carousel(&self) -> &adw::Carousel {
        &self.carousel
    }

    /// The dots, for the bar under the step.
    pub(crate) fn footer(&self) -> &gtk::CenterBox {
        &self.footer
    }

    /// The page before, for the header bar's start; drawn only past the first page.
    pub(crate) fn back(&self) -> &gtk::Button {
        &self.back
    }

    pub(crate) fn index(&self) -> u32 {
        self.index.get()
    }

    pub(crate) fn is_last(&self) -> bool {
        self.index.get() + 1 >= self.carousel.n_pages()
    }

    /// Turns one page forward, or back.
    pub(crate) fn turn(&self, forward: bool) {
        let index = self.index.get();
        let next = if forward {
            (index + 1).min(self.carousel.n_pages().saturating_sub(1))
        } else {
            index.saturating_sub(1)
        };
        if next != index {
            self.index.set(next);
            self.show();
        }
    }

    /// Brings the carousel to the page it keeps. The pages beside it stay laid out, so they are
    /// kept from the keyboard and from assistive technology.
    fn show(&self) {
        let index = self.index.get();
        for position in 0..self.carousel.n_pages() {
            let page = self.carousel.nth_page(position);
            let current = position == index;
            page.set_can_focus(current);
            page.set_can_target(current);
            page.update_state(&[State::Hidden(!current)]);
            if current {
                self.carousel.scroll_to(&page, true);
            }
        }
        self.back.set_visible(index > 0);
    }
}
