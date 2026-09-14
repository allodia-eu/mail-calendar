//! The account-less showcase boot: the dataset a host brings up to photograph the screen somebody
//! meets before they have a mailbox.
//!
//! Its own file rather than a case in `tests_showcase.rs`, which sits against the 500-line limit.

use std::sync::mpsc;

use super::*;
use crate::tests::{ChannelObserver, NullLogger};

#[test]
fn showcase_first_run_holds_no_account() {
    // The first-run boot exists so the onboarding screen can be photographed, and that screen is
    // reached by having no account at all: the offer above the address field is put once and never
    // again to somebody who already has one, so the seeded dataset above cannot show it. An empty
    // account list is therefore the whole contract, and a boot that quietly seeded the usual two
    // would still come up, still render, and photograph the wrong screen.
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_showcase_first_run(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Europe/Amsterdam".to_owned(),
        ShowcaseLocale::En,
    );

    let snapshot = app.mailbox_list();
    assert!(
        snapshot.accounts.is_empty(),
        "the first-run showcase must hold no account, got {}",
        snapshot.accounts.len()
    );
    assert!(
        snapshot.rows.is_empty(),
        "no account means no mail, got {} row(s)",
        snapshot.rows.len()
    );
    // Still a showcase build: the hosts assert on that before they fire the shutter, and detection
    // has to answer from the script rather than the network on this screen of all screens, since it
    // is the one that types an address.
    assert!(app.showcase);
}
