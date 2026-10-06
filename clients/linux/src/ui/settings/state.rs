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
    /// Show the notice raised last over the window on screen, and rebuild nothing: what the
    /// person typed stays as they left it.
    NoticeOnly,
}

/// Everything the model holds about Settings: which generation is pending, what it should show,
/// and whether it is a request to *open* the window or only to redraw an open one.
///
/// Together in one type because they are written and read as one, and because the kind of
/// redraw is an invariant rather than a field. A caller picks [`open`](Self::open),
/// [`refresh`](Self::refresh) or one of their siblings and cannot express anything else; when it
/// was a `bool` beside the generation, two of the four sites that bumped the generation forgot
/// to set it, and a stale `true` silently turned the next *open* into nothing.
#[derive(Debug)]
pub(in crate::ui) struct SettingsState {
    generation: u64,
    category: Category,
    redraw: Redraw,
    /// An Allodia sign-in's browser hop is outstanding, and the failure it may have left. Here
    /// rather than in the window because the window is rebuilt on every change and a hop outlives
    /// several.
    pub(in crate::ui) allodia_signing_in: bool,
    /// Whether that hop has outlasted the first-run card's threshold. Only that card reads
    /// it: the Settings card offers its way out from the first frame.
    pub(in crate::ui) allodia_sign_in_slow: bool,
    pub(in crate::ui) allodia_failure: Option<String>,
    /// What the person's other devices have to say about their mail accounts. Here for the same
    /// reason as the two above: a pass outlives several window rebuilds.
    pub(in crate::ui) allodia_sync: crate::ui::allodia_sync::AllodiaSyncState,
    /// What the subscription section has learned, for the same reason: its read is a network
    /// round trip and its writes are three more.
    pub(in crate::ui) allodia_subscription: crate::ui::allodia_subscription::SubscriptionState,
    /// The account whose page Accounts shows, or `None` for the list. Here because a change on
    /// that page redraws the window, and the page must survive it.
    pub(in crate::ui) account: Option<String>,
    /// What the last change on an account's page came to, when it needs saying.
    pub(in crate::ui) notice: Option<super::notice::Raised>,
    /// How many notices have been raised, so a notice raised after one was dropped never reuses
    /// a number the window has already shown.
    notices_raised: u64,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            // Nothing is pending: generation 0 renders no window.
            generation: 0,
            category: Category::General,
            redraw: Redraw::Open,
            allodia_signing_in: false,
            allodia_sign_in_slow: false,
            allodia_failure: None,
            allodia_sync: crate::ui::allodia_sync::AllodiaSyncState::default(),
            allodia_subscription: crate::ui::allodia_subscription::SubscriptionState::default(),
            account: None,
            notice: None,
            notices_raised: 0,
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
        // Nor with a notice raised while it was closed, about a change made before.
        self.notice = None;
        self.redraw = Redraw::Open;
        self.bump();
    }

    /// Redraws the window if it is open, and leaves a closed one closed.
    ///
    /// How the Allodia card changes: every page is built from the core when the window opens, so
    /// bumping the generation is the whole of "refresh". A sign-in outlives whatever the user does
    /// next, and a redirect landing after they closed Settings must not put it back over their
    /// mail.
    pub(in crate::ui) fn refresh(&mut self, category: Category) {
        self.category = category;
        self.redraw = Redraw::InPlace;
        self.bump();
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

    /// Says `notice` over the window, once, and redraws whatever page is open.
    pub(in crate::ui) fn notify(&mut self, notice: super::notice::Notice) {
        self.raise(notice);
        self.refresh_in_place();
    }

    /// Says `notice` over the window, once, and leaves the page as it is: for progress, and for a
    /// refusal of what the person typed, which they will want to correct rather than retype.
    pub(in crate::ui) fn say(&mut self, notice: super::notice::Notice) {
        self.raise(notice);
        self.redraw = Redraw::NoticeOnly;
        self.bump();
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

    /// What the window is rendered from. `credential_repair_failed` and `accounts_synced` ride
    /// along because they live on the model beside this, and the window reads them together.
    pub(in crate::ui) fn render_state<'a>(
        &'a self,
        credential_repair_failed: Option<&'a str>,
        accounts_synced: &'a std::collections::HashMap<
            String,
            mailcal_bindings::AllodiaAccountSyncMode,
        >,
    ) -> RenderState<'a> {
        RenderState {
            generation: self.generation,
            category: self.category,
            credential_repair_failed,
            allodia_signing_in: self.allodia_signing_in,
            allodia_failure: self.allodia_failure.as_deref(),
            allodia_sync: &self.allodia_sync,
            allodia_subscription: &self.allodia_subscription,
            allodia_accounts_synced: accounts_synced,
            account: self.account.as_deref(),
            notice: self.notice.as_ref(),
            redraw: self.redraw,
        }
    }
}
