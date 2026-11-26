//! A server reached only over plaintext as a notice (`docs/notices.md`): what clears it decides
//! what the notice offers.

use super::{mail_address, oauth, quiet};
use crate::{
    ConnectivitySnapshot, NoticeAction, NoticeKind, NoticeScreens, NoticeSeverity,
    notices::{NoticeBook, NoticeFacts, Questions, raise},
};

/// A server reached only over plaintext stops the account as a dead sign-in does, but no sign-in
/// clears it: a plaintext address opens the account's page, where a secure one is given, and one
/// the server led to plaintext is retried, since only its operator can change that.
#[test]
fn a_server_reached_only_over_plaintext_blocks_the_account_and_says_what_clears_it() {
    use mailcal_account::InsecureCause as Cause;
    let connectivity = ConnectivitySnapshot {
        insecure_server_accounts: vec![
            ("imap".to_owned(), Cause::PlaintextAddress),
            ("jmap".to_owned(), Cause::PlaintextAddress),
            ("led".to_owned(), Cause::LedToPlaintext),
        ],
        ..quiet()
    };
    let facts = NoticeFacts {
        connectivity: &connectivity,
        skipped_at_launch: 0,
        folder_notice: None,
        provider: &oauth,
        edits_servers: &|id| id == "imap",
        address: &mail_address,
        questions: &Questions::default(),
        signing_in: None,
        settings_not_saved: false,
    };
    let notices = NoticeBook::default().snapshot(raise(&facts)).notices;
    let said: Vec<_> = notices
        .iter()
        .map(|notice| {
            (
                notice.account.as_deref(),
                notice.kind.clone(),
                notice.severity,
                notice.screens,
                notice.action,
            )
        })
        .collect();
    let insecure = |cause, address_editable| NoticeKind::InsecureServer {
        cause,
        address_editable,
    };
    let (plaintext, led) = (
        crate::InsecureCause::PlaintextAddress,
        crate::InsecureCause::LedToPlaintext,
    );
    let opens = Some(NoticeAction::OpenAccountSettings);
    let retries = Some(NoticeAction::Retry);
    let (blocked, everywhere) = (NoticeSeverity::Blocked, NoticeScreens::Everywhere);
    assert_eq!(
        said,
        [
            (
                Some("imap"),
                insecure(plaintext, true),
                blocked,
                everywhere,
                opens
            ),
            (
                Some("jmap"),
                insecure(plaintext, false),
                blocked,
                everywhere,
                opens
            ),
            (
                Some("led"),
                insecure(led, false),
                blocked,
                everywhere,
                retries
            ),
        ]
    );
}
