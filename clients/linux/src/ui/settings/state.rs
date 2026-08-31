//! The model's half of Settings: what the next render should show, and whether it opens the
//! window or only redraws an open one.
//!
//! Split from the window itself so each file stays within the 500-line limit, and because the two
//! answer different questions; this one is state the model owns between renders, that one is the
//! GTK window built from it.

use super::{Category, RenderState};

/// Everything the model holds about Settings: which generation is pending, what it should show,
/// and whether it is a request to *open* the window or only to redraw an open one.
///
/// Together in one type because they are written and read as one, and because the open/refresh
/// distinction is an invariant rather than a field. A caller picks [`open`](Self::open) or
/// [`refresh`](Self::refresh) and cannot express anything else; when it was a `bool` beside the
/// generation, two of the four sites that bumped the generation forgot to set it, and a stale
/// `true` silently turned the next *open* into nothing.
#[derive(Debug)]
pub(in crate::ui) struct SettingsState {
    generation: u64,
    category: Category,
    refresh_only: bool,
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
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            // Nothing is pending: generation 0 renders no window.
            generation: 0,
            category: Category::General,
            refresh_only: false,
            allodia_signing_in: false,
            allodia_sign_in_slow: false,
            allodia_failure: None,
            allodia_sync: crate::ui::allodia_sync::AllodiaSyncState::default(),
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
        self.refresh_only = false;
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
        self.refresh_only = true;
        self.bump();
    }

    /// Records the page the window is showing, without redrawing anything.
    ///
    /// The sidebar switches the stack itself, so navigating inside the window never reached the
    /// model; and every model-driven rebuild therefore dropped the person back on whatever page
    /// the model last *named*. Deliberately no `bump`: the widget already shows this page, and a
    /// generation here would rebuild the window under a click.
    pub(in crate::ui) const fn record_category(&mut self, category: Category) {
        self.category = category;
    }

    /// Redraws whatever page is open, without moving the person off it.
    ///
    /// For a change they made **on** that page. Naming a category here instead would take the
    /// Accounts page away mid-gesture and put Allodia in its place, which reads as the app
    /// rejecting what they just did rather than doing it.
    pub(in crate::ui) fn refresh_in_place(&mut self) {
        self.refresh_only = true;
        self.bump();
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
            allodia_accounts_synced: accounts_synced,
            refresh_only: self.refresh_only,
        }
    }
}
