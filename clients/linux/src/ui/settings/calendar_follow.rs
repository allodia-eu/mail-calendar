//! The pages drawn from what the calendars offer (Settings → Calendar's default calendar, Settings
//! → Videoconferencing) follow the core's calendar and settings signals in place.
//!
//! Each redraws only the part of its page whose record changed, so a check or picker the person
//! just used keeps its focus and the Jitsi field keeps what is being typed. The window's own
//! settings signal rebuilds Accounts alone, so without this these pages would show what the
//! calendars offered when the window was built.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use super::SettingsWindow;

/// What brings one part of a page to the core's record.
type Redraw = Rc<dyn Fn()>;

/// The parts of the pages the window drew last that follow the core, and the last read they
/// were asked to follow.
#[derive(Clone, Default)]
pub(super) struct Followers {
    redraws: Rc<RefCell<Vec<Redraw>>>,
    read: Rc<Cell<u64>>,
}

impl std::fmt::Debug for Followers {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Followers")
    }
}

impl Followers {
    /// Adds a part of a page being built. Called once per part, as the page is built.
    pub(super) fn add(&self, redraw: impl Fn() + 'static) {
        self.redraws.borrow_mut().push(Rc::new(redraw));
    }

    /// Forgets the parts of the pages drawn before: the window is about to build them again.
    pub(super) fn clear(&self) {
        self.redraws.borrow_mut().clear();
    }
}

impl SettingsWindow {
    /// Brings the pages drawn from the calendars to the core's records, once for each read the
    /// model counted.
    pub(in crate::ui) fn follow_calendar(&self, read: u64) {
        if self.calendar.read.replace(read) == read {
            return;
        }
        let redraws = self.calendar.redraws.borrow().clone();
        for redraw in redraws {
            redraw();
        }
    }
}
