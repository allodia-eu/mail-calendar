//! First run: the Allodia-account recommendation, above the address field.
//!
//! [`docs/onboarding.md`] is the contract and decides the order: the card, the way back for
//! someone who already has one, a divider naming what follows, then the address field. Its Apple,
//! Android and Windows twins draw the same four things in the same order.
//!
//! Three rules it is easy to break silently:
//!
//! - A build with no Allodia registration loses the card, the sign-in line **and** the divider
//!   together. A lone "or connect directly" heading under nothing is the tell that the wrong thing
//!   was gated.
//! - The copy may not out-run the README capability matrix: phone and computer, never web.
//! - The card claims the account **list** and nothing else; never the mail, never a password.
//!
//! [`docs/onboarding.md`]: https://allodia.eu/docs/mail-calendar

use adw::prelude::*;
use mailcal_bindings::AllodiaAccountOffer;

use super::AppInput;
use crate::l10n;

/// What the first-run card needs, held by the model rather than the window.
///
/// A sign-in leaves for the browser and comes back on a loopback listener, which outlives several
/// window rebuilds; the same reason the Settings card's state lives in the model.
#[derive(Debug, Clone)]
pub(super) struct Onboarding {
    /// Whether this build carries an Allodia registration at all.
    pub(super) offered: bool,
    pub(super) progress: Progress,
    pub(super) failure: Option<String>,
    /// What the last pass answered with, or `None` while none has answered.
    ///
    /// The distinction is the whole of the empty state's correctness: `Some([])` is "this account
    /// has no mail accounts", `None` is "we have not looked", and only the first may say so.
    pub(super) offers: Option<Vec<AllodiaAccountOffer>>,
}

/// How far the person has got with an Allodia account, which is the whole of what the card draws.
///
/// One value rather than three flags, because only one of them can be true: the combinations the
/// flags could express; signing in and signed in at once, checking without having signed in,
/// have no screen behind them.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum Progress {
    /// Nobody has signed in on this device, so the recommendation is what the card says.
    #[default]
    Offering,
    /// The browser has the sign-in. `escapable` once the hop has outlasted
    /// [`SIGN_IN_ESCAPE_AFTER`] and the card owes the person a way back.
    SigningIn { escapable: bool },
    /// Signed in, and the pass that follows is still running.
    Checking,
    /// Signed in and asked; `offers` is the answer, empty or not.
    SignedIn,
}

impl Onboarding {
    /// Nothing offered and nothing asked. `const` because [`super::setup::SetupState::closed`] is,
    /// and a derived `Default` is not.
    pub(super) const fn new() -> Self {
        Self {
            offered: false,
            progress: Progress::Offering,
            failure: None,
            offers: None,
        }
    }
}

impl Default for Onboarding {
    fn default() -> Self {
        Self::new()
    }
}

/// The card, the sign-in line and the divider; or nothing at all.
///
/// Appends to `content` rather than returning a widget, because what it adds is three siblings of
/// the address field below rather than one box around them.
pub(super) fn append(
    content: &gtk::Box,
    state: &Onboarding,
    sender: &relm4::Sender<AppInput>,
    first_run: bool,
) {
    if !state.offered {
        return;
    }
    // What an offer is, and what the card is, part company on the second account. The card is a
    // pitch and is asked once: somebody who signed in has decided. The offers are not a pitch,
    // they are accounts they already have, and hiding them behind "you have decided" is what made
    // the second of three linked accounts reachable only from a Settings page.
    if !first_run {
        if append_offers(content, state.offers.as_deref(), sender) {
            append_divider(content);
        }
        return;
    }
    match state.progress {
        Progress::SigningIn { escapable } => content.append(&busy(
            l10n::settings_allodia_signing_in(),
            escapable.then(|| sender.clone()),
        )),
        // No cancel: this pass is a bounded network call, not a wait on somebody in another
        // application, and nothing it does needs escaping from.
        Progress::Checking => content.append(&busy(l10n::settings_allodia_sync_checking(), None)),
        // Signed in and asked. Offers become the fast route; none means this is their first
        // device, and the address field below is the whole of what is left to do.
        Progress::SignedIn => {
            // A pass that answered with nothing says so; one that has not answered says nothing.
            if !append_offers(content, state.offers.as_deref(), sender) && state.offers.is_some() {
                content.append(&nothing_to_bring_over());
            }
        }
        Progress::Offering => append_recommendation(content, sender),
    }
    if let Some(failure) = &state.failure {
        let error = gtk::Label::new(Some(&l10n::settings_allodia_failed(failure)));
        error.set_xalign(0.0);
        error.set_wrap(true);
        error.add_css_class("error");
        content.append(&error);
    }
    append_divider(content);
}

/// What the address field below is, named. Only ever under something: a lone "or connect directly"
/// heading over nothing is the tell that a client gated the wrong half.
fn append_divider(content: &gtk::Box) {
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let divider = gtk::Label::new(Some(l10n::setup_allodia_divider()));
    divider.set_xalign(0.0);
    divider.add_css_class("dim-label");
    content.append(&divider);
}

/// How long a browser hop runs before the card owes the person a way back.
///
/// Long enough that the ordinary hop; which puts the browser in front of them within a second,
/// never draws a button they had no reason to read. The wait itself is capped far higher, so
/// without this the only way out of a sign-in that went wrong somewhere else is to kill the app.
pub(super) const SIGN_IN_ESCAPE_AFTER: std::time::Duration = std::time::Duration::from_secs(8);

/// A spinner and a line, plus a way back once `cancel` is `Some`.
///
/// The button is added rather than revealed, because a hidden widget is still in the focus walk:
/// one that is not on offer yet must not be reachable by Tab.
fn busy(text: &str, cancel: Option<relm4::Sender<AppInput>>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let spinner = gtk::Spinner::new();
    spinner.start();
    row.append(&spinner);
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dim-label");
    row.append(&label);
    if let Some(sender) = cancel {
        let button = gtk::Button::with_label(l10n::action_cancel());
        button.set_valign(gtk::Align::Center);
        button.connect_clicked(move |_| sender.emit(AppInput::CancelAllodiaSignIn));
        row.append(&button);
    }
    row
}

/// One control rather than a heading beside a button, so a screen reader announces the offer and
/// its action together: and the label carries the **action**, never the "Recommended" marker.
///
/// Neither half is set here, and neither may be: an `AdwActionRow` publishes its title through a
/// `labelled-by` relation and its subtitle through a `described-by` one, and by the ARIA rules GTK
/// follows a relation beats the matching explicit property. Setting either changes nothing a
/// screen reader hears (`AGENTS.md` → Client conventions).
fn append_recommendation(content: &gtk::Box, sender: &relm4::Sender<AppInput>) {
    let group = adw::PreferencesGroup::builder()
        .title(gtk::glib::markup_escape_text(l10n::setup_allodia_recommended()).as_str())
        .build();
    let row = super::mailbox::plain_text_row();
    row.set_title(l10n::setup_allodia_title());
    row.set_subtitle(l10n::setup_allodia_subtitle());
    let create = gtk::Button::with_label(l10n::setup_allodia_create());
    create.add_css_class("suggested-action");
    create.set_valign(gtk::Align::Center);
    let start = sender.clone();
    create.connect_clicked(move |_| start.emit(AppInput::StartAllodiaRegistration));
    row.add_suffix(&create);
    group.add(&row);
    content.append(&group);

    // One line, not a second control of equal weight.
    let sign_in = gtk::Button::with_label(l10n::setup_allodia_have_one());
    sign_in.add_css_class("flat");
    sign_in.set_halign(gtk::Align::Start);
    let start = sender.clone();
    sign_in.connect_clicked(move |_| start.emit(AppInput::StartAllodiaSignIn));
    content.append(&sign_in);
}

/// What a signed-in person is offered, which for a first device is a sentence rather than rows.
///
/// The empty answer is the one worth drawing carefully. Nothing came back, the card is gone, and
/// what is left under the divider is an address field the person has no reason to connect with the
/// sign-in they just finished; it reads as the sign-in having failed. So the empty case says what
/// happened and what to do, and does not leave that to be inferred from a blank space.
/// The accounts the person's other devices hold, as rows. Answers whether it drew any.
///
/// `None` is a pass that has not answered: one that failed, or one still to run: and is not the
/// same as a pass that answered with nothing. Only the caller knows which of the two is worth
/// saying out loud on the screen it is drawing.
fn append_offers(
    content: &gtk::Box,
    offers: Option<&[AllodiaAccountOffer]>,
    sender: &relm4::Sender<AppInput>,
) -> bool {
    let Some(offers) = offers.filter(|offers| !offers.is_empty()) else {
        return false;
    };
    let group = adw::PreferencesGroup::builder()
        .title(gtk::glib::markup_escape_text(l10n::settings_allodia_sync_heading()).as_str())
        .build();
    for offer in offers {
        let row = super::mailbox::plain_text_row();
        row.set_title(&offer.email);
        let set_up = gtk::Button::with_label(l10n::settings_allodia_sync_set_up());
        set_up.add_css_class("suggested-action");
        set_up.set_valign(gtk::Align::Center);
        let sender = sender.clone();
        let offer = offer.clone();
        // The whole record, not the address: the route comes from what the other device wrote
        // down, which is the point of having synced it.
        set_up.connect_clicked(move |_| {
            sender.emit(AppInput::SetUpOfferedAccount(Box::new(offer.clone())));
        });
        row.add_suffix(&set_up);
        group.add(&row);
    }
    content.append(&group);
    true
}

/// Signed in, and this account has no mail accounts on it yet.
///
/// A statement, not a group: there is nothing here to act on, and a heading over an empty list is
/// the shape this is replacing.
fn nothing_to_bring_over() -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let title = gtk::Label::new(Some(l10n::setup_allodia_none_title()));
    title.set_xalign(0.0);
    title.add_css_class("heading");
    column.append(&title);
    let body = gtk::Label::new(Some(l10n::setup_allodia_none_body()));
    body.set_xalign(0.0);
    body.set_wrap(true);
    body.add_css_class("dim-label");
    column.append(&body);
    column
}
