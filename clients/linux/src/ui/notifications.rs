//! New-mail notifications through the desktop portal: one per message, saying who it is from,
//! what it is about and how it begins.

use std::time::Duration;

use ashpd::desktop::notification::{Notification, NotificationProxy, Priority};
use mailcal_bindings::{AccountNewMail, BackgroundSyncOutcome, NewMailPreview};

use crate::{host_runtime, l10n, ui::mailbox_display::one_line};

/// How long the whole portal exchange may take before the pass gives up on it.
///
/// The caller holds the new-mail scan open until this returns, so an unbounded wait would stop
/// the client noticing mail at all, not merely stop it notifying. Generous enough that a busy
/// desktop still delivers, short enough that a portal which has stopped answering costs one pass.
const PORTAL_BUDGET: Duration = Duration::from_secs(10);

pub(super) fn post(outcome: BackgroundSyncOutcome) {
    if outcome.accounts.is_empty() {
        return;
    }
    // Never a runtime of its own: the portal connection is process-global and outlives this call
    // ([`host_runtime`]).
    let Some(runtime) = host_runtime::shared() else {
        log_failure();
        return;
    };
    let posted = runtime
        .block_on(async { tokio::time::timeout(PORTAL_BUDGET, post_all(outcome.accounts)).await });
    if !matches!(posted, Ok(Ok(()))) {
        log_failure();
    }
}

async fn post_all(accounts: Vec<AccountNewMail>) -> ashpd::Result<()> {
    let proxy = NotificationProxy::new().await?;
    for account in accounts {
        for (title, body, id) in notification_parts(&account) {
            proxy
                .add_notification(
                    &id,
                    Notification::new(&title)
                        .body(body.as_str())
                        .priority(Priority::Normal),
                )
                .await?;
        }
    }
    Ok(())
}

fn notification_parts(account: &AccountNewMail) -> Vec<(String, String, String)> {
    let mut parts = account
        .messages
        .iter()
        .map(|message| {
            let title = message
                .sender_name
                .as_deref()
                .filter(|name| !name.trim().is_empty())
                .or_else(|| (!message.sender.trim().is_empty()).then_some(message.sender.as_str()))
                .unwrap_or_else(|| l10n::notification_unknown_sender())
                .to_owned();
            (
                title,
                body_of(message),
                format!("mailcal-{}", message.message_key),
            )
        })
        .collect::<Vec<_>>();
    let hidden = account
        .new_count
        .saturating_sub(u32::try_from(account.messages.len()).unwrap_or(u32::MAX));
    if hidden > 0 {
        // The overflow summary needs copy of its own: reusing the unknown-sender title made a
        // single hidden message read as a sixth subject-less message, and tied two unrelated
        // meanings to one catalog key. "+N more" is the shape the other clients already use.
        parts.push((
            l10n::notification_more_messages(i64::from(hidden)),
            account.account_label.clone(),
            format!("mailcal-account-{}", account.account_id),
        ));
    }
    parts
}

/// Separates the subject from the snippet inside the one body the portal gives us.
///
/// Not a newline: GNOME collapses every run of whitespace in a notification body, expanded view
/// included, so a line break there leaves the subject running into the snippet as one sentence.
/// A separator that survives that reads the same on the daemons which do honour a break. It stays
/// out of the catalog because it is punctuation rather than copy, and it is the one the mailbox
/// already uses.
const BODY_SEPARATOR: &str = " \u{b7} ";

/// What the message is about and how it begins, in the one body the portal gives us: both where
/// the account has a snippet for the message, the subject alone where it does not (an IMAP account
/// has none until the body sync has run). Every other client has a third text slot and uses it;
/// here they share one, which is the same content in the shape this surface allows.
///
/// Both halves are collapsed the way the list row collapses them, so a notification and its row
/// cannot quote the same message differently.
fn body_of(message: &NewMailPreview) -> String {
    [one_line(&message.subject), one_line(&message.preview)]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(BODY_SEPARATOR)
}

fn log_failure() {
    // Covers a portal that answered with an error and one that did not answer inside
    // `PORTAL_BUDGET`: the user sees the same thing either way, and the pass carries on.
    //
    // Deliberately omit the portal error string: a D-Bus implementation may include the
    // notification payload, and diagnostic logs must never contain sender/subject content.
    // It goes through `log` so it reaches the rotating diagnostic file a user attaches to a
    // support request; stderr is lost the moment the app is started from a launcher.
    log::warn!("could not post a desktop notification");
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::{AccountNewMail, NewMailPreview};

    use super::notification_parts;

    #[test]
    fn one_message_uses_sender_subject_and_snippet() {
        let account = AccountNewMail {
            account_id: "account".to_owned(),
            account_label: "account@example.test".to_owned(),
            new_count: 1,
            messages: vec![NewMailPreview {
                sender: "jane@example.test".to_owned(),
                sender_name: Some("Jane".to_owned()),
                subject: "Quarterly report".to_owned(),
                preview: "The numbers you asked for.".to_owned(),
                received: String::new(),
                message_key: "m1".to_owned(),
            }],
        };

        let [(title, body, _)] = notification_parts(&account).try_into().unwrap();
        assert_eq!(title, "Jane");
        assert_eq!(
            body, "Quarterly report \u{b7} The numbers you asked for.",
            "the portal gives one body, so the subject and the snippet share it"
        );
    }

    #[test]
    fn a_snippet_reaches_the_body_in_the_shape_the_list_row_gives_it() {
        // A real snippet carries the body's own line breaks, and a bounce report carries blank
        // lines between paragraphs. The row collapses them and so does this, or the two would
        // quote the same message differently.
        let account = AccountNewMail {
            account_id: "account".to_owned(),
            account_label: "account@example.test".to_owned(),
            new_count: 1,
            messages: vec![NewMailPreview {
                sender: String::new(),
                sender_name: Some("Mail Delivery Subsystem".to_owned()),
                subject: "Failed to deliver message".to_owned(),
                preview: "Your message could not be delivered:\n\n<news@example.com>\n\n"
                    .to_owned(),
                received: String::new(),
                message_key: "m1".to_owned(),
            }],
        };

        let [(_, body, _)] = notification_parts(&account).try_into().unwrap();
        assert_eq!(
            body,
            "Failed to deliver message \u{b7} Your message could not be delivered: \
             <news@example.com>"
        );
    }

    #[test]
    fn a_message_with_no_subject_is_its_snippet_alone() {
        // Never a body opening on the separator: it would read as a subject nobody can see.
        let account = AccountNewMail {
            account_id: "account".to_owned(),
            account_label: "account@example.test".to_owned(),
            new_count: 1,
            messages: vec![NewMailPreview {
                sender: "jane@example.test".to_owned(),
                sender_name: Some("Jane".to_owned()),
                subject: String::new(),
                preview: "The numbers you asked for.".to_owned(),
                received: String::new(),
                message_key: "m1".to_owned(),
            }],
        };

        let [(_, body, _)] = notification_parts(&account).try_into().unwrap();
        assert_eq!(body, "The numbers you asked for.");
    }

    #[test]
    fn a_message_with_no_snippet_yet_is_its_subject_alone() {
        // An IMAP account has no snippet until the body sync has run, and a body ending in a
        // blank line would read as a message that begins with nothing.
        let account = AccountNewMail {
            account_id: "account".to_owned(),
            account_label: "account@example.test".to_owned(),
            new_count: 1,
            messages: vec![NewMailPreview {
                sender: "jane@example.test".to_owned(),
                sender_name: Some("Jane".to_owned()),
                subject: "Quarterly report".to_owned(),
                preview: String::new(),
                received: String::new(),
                message_key: "m1".to_owned(),
            }],
        };

        let [(_, body, _)] = notification_parts(&account).try_into().unwrap();
        assert_eq!(body, "Quarterly report");
    }

    #[test]
    fn multiple_messages_keep_stable_message_notifications_and_summarize_only_the_cap() {
        let account = AccountNewMail {
            account_id: "account".to_owned(),
            account_label: "account@example.test".to_owned(),
            new_count: 3,
            messages: vec![
                NewMailPreview {
                    sender: "one@example.test".to_owned(),
                    sender_name: None,
                    subject: "One".to_owned(),
                    preview: String::new(),
                    received: String::new(),
                    message_key: "m1".to_owned(),
                },
                NewMailPreview {
                    sender: "two@example.test".to_owned(),
                    sender_name: None,
                    subject: "Two".to_owned(),
                    preview: String::new(),
                    received: String::new(),
                    message_key: "m2".to_owned(),
                },
            ],
        };

        let parts = notification_parts(&account);
        assert_eq!(parts[0].2, "mailcal-m1");
        assert_eq!(parts[1].2, "mailcal-m2");
        assert_eq!(
            parts[2].0, "+1 more",
            "the overflow summary must not read like another subject-less message"
        );
        assert_eq!(parts[2].1, "account@example.test");
    }
}
