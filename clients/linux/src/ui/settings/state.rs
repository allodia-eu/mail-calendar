//! The model's half of Settings: what the next render should show, and whether it opens the
//! window or only redraws an open one.
//!
//! Split from the window itself so each file stays within the 500-line limit, and because the two
//! answer different questions; this one is state the model owns between renders, that one is the
//! GTK window built from it.

use super::{Category, RenderState};

/// What a generation asks of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui) enum Redraw {
    /// Put the window on screen.
    Open,
    /// Redraw the window if it is open, and leave a closed one closed.
    ///
    /// Without this, an Allodia redirect landing after the person closed Settings would put the
    /// window back on screen over their mail.
    InPlace,
    /// As [`InPlace`](Self::InPlace), for the core's signal rather than a change made in the
    /// window: skipped when the accounts are as drawn, and held while the person is typing.
    Signalled,
}

/// Everything the model holds about Settings: which generation is pending, what it should show,
/// and whether it is a request to *open* the window or only to redraw an open one.
///
/// Together in one type because they are written and read as one, and because the kind of
/// redraw is an invariant rather than a field. A caller picks [`open`](Self::open),
/// [`refresh_in_place`](Self::refresh_in_place) or one of their siblings and cannot express anything else; when it
/// was a `bool` beside the generation, two of the four sites that bumped the generation forgot
/// to set it, and a stale `true` silently turned the next *open* into nothing.
#[derive(Debug)]
pub(in crate::ui) struct SettingsState {
    generation: u64,
    category: Category,
    redraw: Redraw,
    /// Who the Allodia card last drew as signed in: the subscription is forgotten when it moves.
    pub(in crate::ui) allodia_seen: Option<String>,
    /// Whether a sign-in was out when the card was last read: one ending reads the subscription
    /// again.
    pub(in crate::ui) allodia_signing_in: bool,
    /// The last of the service's sign-out pages opened, so each is opened once.
    pub(in crate::ui) allodia_end_session: u64,
    /// What the subscription section has learned, for the same reason: its read is a network
    /// round trip and its writes are three more.
    pub(in crate::ui) allodia_subscription: crate::ui::allodia_subscription::SubscriptionState,
    /// The account whose page Accounts shows, or `None` for the list. Here because a change on
    /// that page redraws the window, and the page must survive it.
    pub(in crate::ui) account: Option<String>,
    /// The form for a sign-in of its own that an account's page opened, while it is open.
    pub(in crate::ui) separate: Option<crate::ui::separate_sign_in::SeparateForm>,
    /// How many such forms have been opened, so each is told from the one before it.
    pub(in crate::ui) separate_forms: u64,
    /// What the last change on an account's page came to, when it needs saying.
    pub(in crate::ui) notice: Option<super::notice::Raised>,
    /// How many notices have been raised, so a notice raised after one was dropped never reuses
    /// a number the window has already shown.
    notices_raised: u64,
    /// The last of the core's account change notices said, so each is said once.
    pub(in crate::ui) core_notice: u64,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            // Nothing is pending: generation 0 renders no window.
            generation: 0,
            category: Category::General,
            redraw: Redraw::Open,
            allodia_seen: None,
            allodia_signing_in: false,
            allodia_end_session: 0,
            allodia_subscription: crate::ui::allodia_subscription::SubscriptionState::default(),
            account: None,
            separate: None,
            separate_forms: 0,
            notice: None,
            notices_raised: 0,
            core_notice: 0,
        }
    }
}

impl SettingsState {
    /// Puts the window on screen: on `category`, or on whatever was last asked for when the
    /// caller names none (the Settings button, which names no category).
    pub(in crate::ui) fn open(&mut self, category: Option<Category>) {
        if let Some(category) = category {
            self.category = category;
        }
        // A window opened afresh starts on the accounts list, not on a page left open earlier.
        self.account = None;
        self.separate = None;
        // Nor with a notice raised while it was closed, about a change made before.
        self.notice = None;
        self.redraw = Redraw::Open;
        self.bump();
    }

    /// Puts the window on screen at an account's page in Accounts.
    pub(in crate::ui) fn open_account(&mut self, account: String) {
        self.open(Some(Category::Accounts));
        self.account = Some(account);
    }

    /// Records the page the window is showing, without redrawing anything.
    ///
    /// The sidebar switches the stack itself, so navigating inside the window never reached the
    /// model; and every model-driven rebuild therefore dropped the person back on whatever page
    /// the model last *named*. Deliberately no `bump`: the widget already shows this page, and a
    /// generation here would rebuild the window under a click.
    /// Redraws the window in place when it shows `category`, and otherwise leaves it alone: for
    /// state only that page draws. A rebuild of another page would close a dropdown the person
    /// just opened and take what they typed, for nothing that page shows.
    pub(in crate::ui) fn refresh_if_showing(&mut self, category: Category) {
        if self.category == category {
            self.refresh_in_place();
        }
    }

    pub(in crate::ui) const fn record_category(&mut self, category: Category) {
        self.category = category;
    }

    /// Redraws whatever page is open, without moving the person off it.
    ///
    /// For a change they made **on** that page. Naming a category here instead would take the
    /// Accounts page away mid-gesture and put Allodia in its place, which reads as the app
    /// rejecting what they just did rather than doing it.
    pub(in crate::ui) fn refresh_in_place(&mut self) {
        self.redraw = Redraw::InPlace;
        self.bump();
    }

    /// The core's settings signal: redraws Accounts if it is the page on screen, whose rows the
    /// signal may describe. Every other page reconciles itself, and a rebuild under one would
    /// take whatever is being written in it.
    pub(in crate::ui) fn signalled(&mut self) {
        if self.category == Category::Accounts {
            self.redraw = Redraw::Signalled;
            self.bump();
        }
    }

    /// Opens the window on `account`'s page, saying `notice` over it: for something about the
    /// account that the page is where to act on.
    pub(in crate::ui) fn open_on_account(
        &mut self,
        account: String,
        notice: super::notice::Notice,
    ) {
        self.open_account(account);
        self.raise(notice);
    }

    /// Says `notice` over the window, once, and redraws whatever page is open.
    pub(in crate::ui) fn notify(&mut self, notice: super::notice::Notice) {
        self.raise(notice);
        self.refresh_in_place();
    }

    fn raise(&mut self, notice: super::notice::Notice) {
        self.notices_raised = self.notices_raised.wrapping_add(1);
        self.notice = Some(super::notice::Raised {
            id: self.notices_raised,
            notice,
        });
    }

    fn bump(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// What the window is rendered from.
    pub(in crate::ui) fn render_state(&self) -> RenderState<'_> {
        RenderState {
            generation: self.generation,
            category: self.category,
            allodia_subscription: &self.allodia_subscription,
            account: self.account.as_deref(),
            separate: self.separate.as_ref(),
            notice: self.notice.as_ref(),
            redraw: self.redraw,
        }
    }
}
