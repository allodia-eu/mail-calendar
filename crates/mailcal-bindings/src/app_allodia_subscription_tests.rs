// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! What a client sees of the account screen, asserted where a client sees it.
//!
//! The parsing rules are tested in `allodia-license`, which needs no network. What is tested here
//! is the half that crosses the FFI: that a build with no registration offers none of it, that a
//! signed-out one says so rather than failing obscurely, and that every refusal the service can
//! send survives as a reason a client can switch on.

use std::sync::mpsc;

use crate::{
    LogLevel, MailcalApp,
    allodia_purchase::{AllodiaPlan, AllodiaPurchaseError},
    tests::{ChannelObserver, NullLogger},
};

fn app() -> std::sync::Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_demo(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    )
}

/// Every one of the five is refused the same way when there is nobody to act for, and none of them
/// reaches the network to find that out.
///
/// A build with no Allodia registration says [`AllodiaPurchaseError::Unavailable`], which is the
/// ordinary answer for every build from source; one that has a registration but nobody signed in
/// says [`AllodiaPurchaseError::NotSignedIn`]. Either way the screen draws nothing rather than a
/// row of buttons that dead-end.
#[test]
fn every_call_refuses_before_it_reaches_the_network() {
    let app = app();
    let refusals = [
        app.allodia_subscription().err(),
        app.start_allodia_checkout(AllodiaPlan::Yearly).err(),
        app.cancel_allodia_subscription().err(),
        app.switch_allodia_interval(AllodiaPlan::Monthly).err(),
        app.resubscribe_to_allodia().err(),
    ];

    for refused in refusals {
        let refused = refused.expect("nothing here can act yet");
        #[cfg(not(feature = "allodia-license"))]
        assert!(matches!(refused, AllodiaPurchaseError::Unavailable));
        #[cfg(feature = "allodia-license")]
        assert!(
            matches!(refused, AllodiaPurchaseError::NotSignedIn),
            "{refused:?}"
        );
    }
}

/// Every refusal the service documents survives the trip as something a client can switch on, and
/// none of them carries the service's own sentence.
///
/// The words a person reads belong to the client: "you have already cancelled" and "that cannot be
/// changed while a charge is being retried" are different sentences, and a client can only write
/// them from a code.
#[cfg(feature = "allodia-license")]
#[test]
fn every_documented_refusal_crosses_as_a_reason_and_not_as_words() {
    use allodia_license::Refusal;

    use crate::allodia_subscription::AllodiaSubscriptionRefusal as Reason;

    for (refusal, expected) in [
        (Refusal::AlreadyCancelled, Reason::AlreadyCancelled),
        (Refusal::AlreadyActive, Reason::AlreadyActive),
        (Refusal::AlreadyOnInterval, Reason::AlreadyOnInterval),
        (Refusal::NotSwitchable, Reason::NotSwitchable),
        (Refusal::NotFound, Reason::NotFound),
        (Refusal::Unavailable, Reason::Unavailable),
        (
            Refusal::Other("some_later_reason".to_owned()),
            Reason::Other,
        ),
    ] {
        assert_eq!(Reason::from(refusal), expected);
    }
}

/// A refusal renders as the app's own words, never the service's. What a client switches on is the
/// reason beside it.
#[cfg(feature = "allodia-license")]
#[test]
fn a_refusal_never_prints_anything_the_service_wrote() {
    use crate::allodia_subscription::AllodiaSubscriptionRefusal as Reason;

    let refused = AllodiaPurchaseError::Refused {
        reason: Reason::AlreadyCancelled,
    };

    assert_eq!(format!("{refused}"), "the account service declined");
}

/// The whole point of `actions`: the service decides, and a client draws exactly that. Asserting
/// the shape crosses intact is what stops a client quietly reading the wrong flag and offering a
/// second subscription to somebody a store already charges.
#[cfg(feature = "allodia-license")]
#[test]
fn what_the_caller_may_do_crosses_field_for_field() {
    use allodia_license::Actions;

    use crate::allodia_subscription::AllodiaSubscriptionActions;

    let drawn = AllodiaSubscriptionActions::from(Actions {
        can_cancel: true,
        can_resubscribe: false,
        can_switch_interval: true,
        can_start_checkout: false,
    });

    assert!(drawn.can_cancel);
    assert!(!drawn.can_resubscribe);
    assert!(drawn.can_switch_interval);
    assert!(!drawn.can_start_checkout);
}

/// Resubscribing has two shapes and a client draws both, so one record carries them: restarted on
/// the authorisation already held, or a page to open.
#[cfg(feature = "allodia-license")]
#[test]
fn resubscribing_crosses_as_either_restarted_or_a_page() {
    use allodia_license::{Checkout, Resubscribed};

    use crate::allodia_subscription::AllodiaCheckout;

    let restarted = AllodiaCheckout::from(Resubscribed {
        reactivated: true,
        checkout_url: None,
    });
    assert!(restarted.reactivated);
    assert!(restarted.checkout_url.is_none());

    // A fresh checkout is never "reactivated": nobody had an authorisation to reuse.
    let fresh = AllodiaCheckout::from(Checkout {
        checkout_url: "https://pay.example.com/abc".to_owned(),
    });
    assert!(!fresh.reactivated);
    assert_eq!(
        fresh.checkout_url.as_deref(),
        Some("https://pay.example.com/abc")
    );
}

/// A grant the service refused outright, signed in on this device and alive nowhere else.
#[cfg(feature = "allodia-license")]
fn signed_in(app: &MailcalApp) {
    *app.allodia.lock().unwrap() = Some(crate::allodia::StoredAccount {
        id: None,
        email: "person@example.test".to_owned(),
        name: None,
        refresh_token: "RT".to_owned(),
        granted_scopes: None,
        end_session_endpoint: None,
    });
}

/// A token the service would not mint because it refused the grant is a sign-in to make again,
/// and the account screen says so; it is never the outage sentence, which tells somebody to wait
/// for something that waiting cannot bring back.
#[cfg(feature = "allodia-license")]
#[test]
fn a_refused_grant_is_signed_out_and_not_an_outage() {
    use crate::AllodiaGrantHealth;

    let app = app();
    signed_in(&app);
    app.note_allodia_health(AllodiaGrantHealth::SignedOut);

    assert!(matches!(
        app.allodia_mint_failure(),
        AllodiaPurchaseError::SignedOut
    ));
}

/// A refresh refused for a scope the grant never had (`invalid_scope`) is the offer to sign in
/// again, never the outage: a client switches on this error alone, so an `Unreachable` here would
/// tell somebody to wait for a permission that waiting cannot bring.
#[cfg(feature = "allodia-license")]
#[test]
fn a_narrowed_grant_that_could_not_refresh_offers_a_fresh_sign_in() {
    use crate::AllodiaGrantHealth;

    let app = app();
    signed_in(&app);
    app.note_allodia_health(AllodiaGrantHealth::NeedsReauth);

    assert!(matches!(
        app.allodia_mint_failure(),
        AllodiaPurchaseError::NeedsReauth
    ));
}

/// A mint that failed with nothing learned about the grant is the outage it looks like.
#[cfg(feature = "allodia-license")]
#[test]
fn a_mint_that_learned_nothing_is_an_outage() {
    let app = app();
    signed_in(&app);

    assert!(matches!(
        app.allodia_mint_failure(),
        AllodiaPurchaseError::Unreachable
    ));
}

/// A token the service took and then refused at the API is the same answer as one it would not
/// mint: the grant is gone.
#[cfg(feature = "allodia-license")]
#[test]
fn a_token_the_api_refused_is_signed_out() {
    use crate::AllodiaGrantHealth;

    let app = app();
    signed_in(&app);

    let drawn = app.classify_allodia_failure(&allodia_license::Error::Unauthorized);

    assert!(matches!(drawn, AllodiaPurchaseError::SignedOut));
    assert_eq!(app.allodia_grant_health(), AllodiaGrantHealth::SignedOut);
}

/// Signing out leaves no grant to have an opinion about, so the next sign-in starts from nothing
/// known to be wrong rather than inheriting the refusal of the grant it replaces.
#[cfg(feature = "allodia-license")]
#[test]
fn signing_out_forgets_what_was_known_about_the_grant() {
    use crate::AllodiaGrantHealth;

    let app = app();
    signed_in(&app);
    app.note_allodia_health(AllodiaGrantHealth::SignedOut);

    // The demo store refuses the erase; the account is forgotten in memory either way.
    let _ = app.sign_out_of_allodia();

    assert_eq!(app.allodia_grant_health(), AllodiaGrantHealth::Ok);
}

/// Whether a connectivity signal reaches the host within a moment, draining everything sent.
#[cfg(feature = "allodia-license")]
fn connectivity_signalled(rx: &mpsc::Receiver<crate::Surface>) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut seen = false;
    while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(crate::Surface::Connectivity) => seen = true,
            Ok(_) => {}
            Err(_) => break,
        }
        if seen {
            break;
        }
    }
    seen
}

/// A refused sign-in is said where the person already looks, beside the mail accounts' own sign-in
/// prompts, and not only on a Settings page nobody opens unprompted. Signing out takes it down,
/// because there is then no sign-in to have ended.
#[cfg(feature = "allodia-license")]
#[test]
fn a_refused_sign_in_raises_the_banner_and_signing_out_takes_it_down() {
    use crate::AllodiaGrantHealth;

    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_demo(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    );
    signed_in(&app);
    assert_eq!(app.connectivity().allodia_sign_in_ended, None);
    while rx.try_recv().is_ok() {}

    app.note_allodia_health(AllodiaGrantHealth::SignedOut);
    assert_eq!(
        app.connectivity().allodia_sign_in_ended.as_deref(),
        Some("person@example.test"),
    );
    assert!(connectivity_signalled(&rx), "the host is told to redraw");

    let _ = app.sign_out_of_allodia();
    assert_eq!(app.connectivity().allodia_sign_in_ended, None);
    assert!(connectivity_signalled(&rx), "and told again when it clears");
}

/// A grant that is alive but narrower than this build wants is an offer on the account screen,
/// never a standing banner.
#[cfg(feature = "allodia-license")]
#[test]
fn a_narrowed_grant_raises_no_banner() {
    use crate::AllodiaGrantHealth;

    let app = app();
    signed_in(&app);
    app.note_allodia_health(AllodiaGrantHealth::NeedsReauth);

    assert_eq!(app.connectivity().allodia_sign_in_ended, None);
}
