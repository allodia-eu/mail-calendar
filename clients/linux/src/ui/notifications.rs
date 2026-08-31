//! New-mail notifications through the desktop portal.

use ashpd::desktop::notification::{Notification, NotificationProxy, Priority};
use mailcal_bindings::{AccountNewMail, BackgroundSyncOutcome};

use crate::l10n;

pub(super) fn post(outcome: BackgroundSyncOutcome) {
    if outcome.accounts.is_empty() {
        return;
    }
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    if runtime.block_on(post_all(outcome.accounts)).is_err() {
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
                message.subject.clone(),
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

fn log_failure() {
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
    fn one_message_uses_sender_and_subject() {
        let account = AccountNewMail {
            account_id: "account".to_owned(),
            account_label: "account@example.test".to_owned(),
            new_count: 1,
            messages: vec![NewMailPreview {
                sender: "jane@example.test".to_owned(),
                sender_name: Some("Jane".to_owned()),
                subject: "Quarterly report".to_owned(),
                received: String::new(),
                message_key: "m1".to_owned(),
            }],
        };

        let [(title, body, _)] = notification_parts(&account).try_into().unwrap();
        assert_eq!(title, "Jane");
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
                    received: String::new(),
                    message_key: "m1".to_owned(),
                },
                NewMailPreview {
                    sender: "two@example.test".to_owned(),
                    sender_name: None,
                    subject: "Two".to_owned(),
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
