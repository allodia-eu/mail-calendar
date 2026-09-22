//! New-mail notifications through the desktop portal: one per message, saying who it is from,
//! what it is about and how it begins, and opening that message when it is clicked.
//!
//! The portal gives one **default action** per notification plus one value to carry back with it,
//! so the message a click names travels in the notification itself, exactly as it does in macOS's
//! `userInfo`, Android's intent extras and Windows's toast arguments
//! (`docs/background-sync.md`). What the click then does to the mailbox is
//! [`super::notification_open`]; this module is the portal, and knows nothing about the list.

use std::time::Duration;

use ashpd::desktop::notification::{Action, Notification, NotificationProxy, Priority};
use futures::StreamExt;
use mailcal_bindings::{AccountNewMail, BackgroundSyncOutcome, NewMailPreview};

use super::{AppInput, mailbox_display::one_line};
use crate::{host_runtime, l10n};

/// How long the whole portal exchange may take before the pass gives up on it.
///
/// The caller holds the new-mail scan open until this returns, so an unbounded wait would stop
/// the client noticing mail at all, not merely stop it notifying. Generous enough that a busy
/// desktop still delivers, short enough that a portal which has stopped answering costs one pass.
const PORTAL_BUDGET: Duration = Duration::from_secs(10);

/// The default action a message notification carries, and the one the overflow summary carries.
///
/// Told apart by name rather than by the notification id: an id is formatted around a message
/// key, and a key is arbitrary provider text, so any prefix that distinguished a summary's id
/// from a message's is one a key is allowed to start with.
const OPEN_MESSAGE: &str = "open-message";
const OPEN_APP: &str = "open-app";

/// The message a clicked notification names: the `(account, message key)` pair the core keys
/// mail on, which is what `Intent::OpenMessage` takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NotificationTarget {
    pub(crate) account: String,
    pub(crate) key: String,
}

/// One notification: what it says, what the portal replaces it by, and the message it opens.
struct Notice {
    title: String,
    body: String,
    id: String,
    /// The message a click opens, or `None` for the overflow summary: it stands for messages it
    /// does not name, so it has none to open and brings the app forward alone.
    target: Option<NotificationTarget>,
}

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

/// Watches the portal for clicks on the notifications this client posted, for as long as the
/// process runs.
///
/// One subscription, taken at startup rather than beside each post: `ActionInvoked` is the
/// portal's own signal rather than one notification's, so a subscription per pass would answer a
/// single click once for every pass that had ever run.
///
/// A click can only arrive while this process is alive, and that is the whole reach of the
/// feature here: the desktop notifies from the live runtime, so the app was running when it
/// posted, and quitting in between leaves a notification on screen with nothing to answer it
/// (`docs/background-sync.md`, known gaps).
pub(super) fn listen(sender: relm4::Sender<AppInput>) {
    let Some(runtime) = host_runtime::shared() else {
        log_failure();
        return;
    };
    runtime.spawn(async move {
        let watched = async {
            let proxy = NotificationProxy::new().await?;
            let actions = proxy.receive_action_invoked().await?;
            let mut actions = std::pin::pin!(actions);
            while let Some(action) = actions.next().await {
                if let Some(input) = input_for(&action) {
                    sender.emit(input);
                }
            }
            Ok::<(), ashpd::Error>(())
        };
        if watched.await.is_err() {
            // Deliberately as bare as the failure to post, and for the same reason: a D-Bus
            // error may quote the notification, which is sender and subject (docs/logging.md).
            log::warn!("could not watch for desktop notification clicks");
        }
    });
}

/// What a click means to the mailbox, or `None` for an action this client did not put there.
///
/// A target that will not decode still names this app, so the click brings the window forward
/// rather than doing nothing at all.
fn input_for(action: &Action) -> Option<AppInput> {
    match action.name() {
        OPEN_MESSAGE => Some(
            target_of(action).map_or(AppInput::ShowFromNotification, |target| {
                AppInput::OpenNotifiedMessage(Box::new(target))
            }),
        ),
        OPEN_APP => Some(AppInput::ShowFromNotification),
        _ => None,
    }
}

/// The pair [`post_all`] sent as the action's target, read back.
fn target_of(action: &Action) -> Option<NotificationTarget> {
    let carried = action.parameter().first()?.try_clone().ok()?;
    let [account, key] = <[String; 2]>::try_from(Vec::<String>::try_from(carried).ok()?).ok()?;
    (!account.is_empty() && !key.is_empty()).then_some(NotificationTarget { account, key })
}

async fn post_all(accounts: Vec<AccountNewMail>) -> ashpd::Result<()> {
    let proxy = NotificationProxy::new().await?;
    for account in accounts {
        for notice in notices(&account) {
            let notification = Notification::new(&notice.title)
                .body(notice.body.as_str())
                .priority(Priority::Normal);
            let notification = match notice.target {
                Some(target) => notification
                    .default_action(OPEN_MESSAGE)
                    .default_action_target(vec![target.account, target.key]),
                None => notification.default_action(OPEN_APP),
            };
            proxy.add_notification(&notice.id, notification).await?;
        }
    }
    Ok(())
}

fn notices(account: &AccountNewMail) -> Vec<Notice> {
    let mut notices = account
        .messages
        .iter()
        .map(|message| Notice {
            title: message
                .sender_name
                .as_deref()
                .filter(|name| !name.trim().is_empty())
                .or_else(|| (!message.sender.trim().is_empty()).then_some(message.sender.as_str()))
                .unwrap_or_else(|| l10n::notification_unknown_sender())
                .to_owned(),
            body: body_of(message),
            id: format!("mailcal-{}", message.message_key),
            target: Some(NotificationTarget {
                account: account.account_id.clone(),
                key: message.message_key.clone(),
            }),
        })
        .collect::<Vec<_>>();
    let hidden = account
        .new_count
        .saturating_sub(u32::try_from(account.messages.len()).unwrap_or(u32::MAX));
    if hidden > 0 {
        // The overflow summary needs copy of its own: reusing the unknown-sender title made a
        // single hidden message read as a sixth subject-less message, and tied two unrelated
        // meanings to one catalog key. "+N more" is the shape the other clients already use.
        notices.push(Notice {
            title: l10n::notification_more_messages(i64::from(hidden)),
            body: account.account_label.clone(),
            id: format!("mailcal-account-{}", account.account_id),
            target: None,
        });
    }
    notices
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

    use super::notices;

    fn one(
        subject: &str,
        preview: &str,
        sender: &str,
        sender_name: Option<&str>,
    ) -> AccountNewMail {
        AccountNewMail {
            account_id: "account".to_owned(),
            account_label: "account@example.test".to_owned(),
            new_count: 1,
            messages: vec![NewMailPreview {
                sender: sender.to_owned(),
                sender_name: sender_name.map(ToOwned::to_owned),
                subject: subject.to_owned(),
                preview: preview.to_owned(),
                received: String::new(),
                message_key: "m1".to_owned(),
            }],
        }
    }

    #[test]
    fn one_message_uses_sender_subject_and_snippet() {
        let account = one(
            "Quarterly report",
            "The numbers you asked for.",
            "jane@example.test",
            Some("Jane"),
        );

        let notices = notices(&account);
        assert_eq!(notices.len(), 1);
        let notice = &notices[0];
        assert_eq!(notice.title, "Jane");
        assert_eq!(
            notice.body, "Quarterly report \u{b7} The numbers you asked for.",
            "the portal gives one body, so the subject and the snippet share it"
        );
    }

    #[test]
    fn a_snippet_reaches_the_body_in_the_shape_the_list_row_gives_it() {
        // A real snippet carries the body's own line breaks, and a bounce report carries blank
        // lines between paragraphs. The row collapses them and so does this, or the two would
        // quote the same message differently.
        let account = one(
            "Failed to deliver message",
            "Your message could not be delivered:\n\n<news@example.com>\n\n",
            "",
            Some("Mail Delivery Subsystem"),
        );

        let notices = notices(&account);
        assert_eq!(notices.len(), 1);
        let notice = &notices[0];
        assert_eq!(
            notice.body,
            "Failed to deliver message \u{b7} Your message could not be delivered: \
             <news@example.com>"
        );
    }

    #[test]
    fn a_message_with_no_subject_is_its_snippet_alone() {
        // Never a body opening on the separator: it would read as a subject nobody can see.
        let account = one(
            "",
            "The numbers you asked for.",
            "jane@example.test",
            Some("Jane"),
        );

        let notices = notices(&account);
        assert_eq!(notices.len(), 1);
        let notice = &notices[0];
        assert_eq!(notice.body, "The numbers you asked for.");
    }

    #[test]
    fn a_message_with_no_snippet_yet_is_its_subject_alone() {
        // An IMAP account has no snippet until the body sync has run, and a body ending in a
        // blank line would read as a message that begins with nothing.
        let account = one("Quarterly report", "", "jane@example.test", Some("Jane"));

        let notices = notices(&account);
        assert_eq!(notices.len(), 1);
        let notice = &notices[0];
        assert_eq!(notice.body, "Quarterly report");
    }

    #[test]
    fn a_message_notification_names_the_message_a_click_opens() {
        // The pair the core keys mail on, and the pair `Intent::OpenMessage` takes. Keyed by
        // message rather than by account for the same reason the notification itself is
        // (docs/background-sync.md).
        let account = one("Quarterly report", "", "jane@example.test", Some("Jane"));

        let notices = notices(&account);
        assert_eq!(notices.len(), 1);
        let notice = &notices[0];
        let target = notice
            .target
            .as_ref()
            .expect("a message notification names its message");
        assert_eq!(target.account, "account");
        assert_eq!(target.key, "m1");
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

        let notices = notices(&account);
        assert_eq!(notices[0].id, "mailcal-m1");
        assert_eq!(notices[1].id, "mailcal-m2");
        assert_eq!(
            notices[2].title, "+1 more",
            "the overflow summary must not read like another subject-less message"
        );
        assert_eq!(notices[2].body, "account@example.test");
        assert!(
            notices[2].target.is_none(),
            "a summary stands for messages it does not name, so it opens the app alone"
        );
    }
}
