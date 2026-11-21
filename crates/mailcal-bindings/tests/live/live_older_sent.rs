//! Gated live check of learning from sent mail older than the account's sync window
//! (`docs/ai.md` rules 11 and 14), over JMAP and over IMAP: a message alice sent 240 days ago,
//! before her three-month window, is fetched for the review, found by it, and removed from the
//! device once the review has read it, while the server keeps it. Two messages are planted in
//! turn: a reply long enough to learn from, and a short one without `MIME-Version`,
//! `In-Reply-To` or `References`, which is found and not usable.
//!
//! Each is planted in alice's `Sent Items` over JMAP (`Email/import`, whose `receivedAt` is the
//! IMAP `INTERNALDATE`) and destroyed again however the test ends. Nothing is sent.
//!
//! Skips unless `STALWART_HTTP_ADDR` is set; the IMAP half also needs `MAILCAL_HARNESS_IMAP` and
//! `MAILCAL_EXTRA_CA`. `scripts/dev/harness.sh test` sets all three. By hand:
//! ```sh
//! scripts/dev/harness.sh up
//! STALWART_HTTP_ADDR=127.0.0.1:28080 MAILCAL_HARNESS_IMAP=localhost:12993 \
//!   MAILCAL_EXTRA_CA=docker/stalwart/tls/harness-ca.pem \
//!   cargo test -p mailcal-bindings --test live -- live_older_sent:: --nocapture
//! ```

mod jmap;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use mailcal_bindings::{
    AiEndpointIntent, AiIntent, FolderRole, Intent, JmapSetup, JurisdictionClass, MailcalApp,
    SnapshotRow, StyleLearnRange, StyleLearningIntent, StyleLearningReport, StyleLearningReview,
    StyleLearningStage, ViewMode, WritingStyleIntent,
};
use serde_json::json;
use time::{OffsetDateTime, Time};

use self::jmap::Jmap;
use crate::live_oauth::{LOGIN, PASSWORD, RecordingStore, app};

/// The token the long message's subject carries, so a rerun can find an earlier run's residue.
const MARKER: &str = "older-sent-live";

/// The short message's subject, as a hand check planted it.
const SHORT_SUBJECT: &str = "Re: Spring planning (older reply)";

/// How long ago the planted message was sent: before the account's three-month window.
const DAYS_AGO: i64 = 240;

/// Which message a run plants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// A reply long enough to learn from, with every header a client writes.
    Long,
    /// A reply of seventeen words with no `MIME-Version`, `In-Reply-To` or `References`, the
    /// exact message a hand check on macOS planted over IMAP. Under forty words of its own.
    Short,
}

impl Shape {
    /// Whether a Sent message with this subject and `Message-ID` is one a run of this shape
    /// planted.
    fn planted(self, subject: &str, message_id: &str) -> bool {
        match self {
            Self::Long => subject.contains(MARKER),
            Self::Short => {
                subject == SHORT_SUBJECT
                    && message_id.starts_with("older-sent-")
                    && !message_id.starts_with(MARKER)
            }
        }
    }

    fn raw(self, transport: &str, date: &str, unix: i64) -> String {
        match self {
            Self::Long => format!(
                "From: Alice <alice@test.local>\r\nTo: Ruth Jansen <ruth@test.local>\r\n\
                 Subject: {MARKER} {transport} notes for the spring board meeting\r\n\
                 Date: {date}\r\nMessage-ID: <{MARKER}-{transport}-{unix}@test.local>\r\n\
                 MIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n\
                 Dear Ruth,\r\n\r\nThank you for sending the draft agenda so early. I have read \
                 it through twice and I think the order works well, although I would move the \
                 budget item before the break so that we are not rushed at the end of the \
                 afternoon.\r\n\r\nCould you also ask the treasurer to bring the printed figures? \
                 Several members asked for them last time.\r\n\r\nKind regards,\r\nAlice\r\n"
            ),
            Self::Short => format!(
                "From: alice@test.local\r\nTo: bob@test.local\r\nSubject: {SHORT_SUBJECT}\r\n\
                 Date: {date}\r\nMessage-ID: <older-sent-{unix}@test.local>\r\n\
                 Content-Type: text/plain; charset=utf-8\r\n\r\nHi Bob,\r\n\r\nThanks for the \
                 plan. Thursday works for me; I will bring the figures.\r\n\r\nBest regards,\r\n\
                 Alice\r\n"
            ),
        }
    }
}

/// Waits up to two minutes for `check` to answer something, polling.
fn eventually<T>(what: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Some(found) = check() {
            return found;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// The message planted for one run, destroyed on the server however the run ends.
struct Planted {
    jmap: Jmap,
    id: String,
    shape: Shape,
    /// When it was sent, in seconds since the Unix epoch.
    sent_at: i64,
}

impl Planted {
    fn plant(addr: &str, transport: &str, shape: Shape) -> Self {
        let jmap = Jmap::connect(addr);
        jmap.destroy(&jmap.planted(|subject, id| shape.planted(subject, id)));
        let at = (OffsetDateTime::now_utc() - time::Duration::days(DAYS_AGO))
            .replace_time(Time::from_hms(9, 0, 0).expect("a time"));
        let three = |name: String| name[..3].to_owned();
        let date = format!(
            "{}, {:02} {} {} 09:00:00 +0000",
            three(at.weekday().to_string()),
            at.day(),
            three(at.month().to_string()),
            at.year(),
        );
        let received_at = format!(
            "{:04}-{:02}-{:02}T09:00:00Z",
            at.year(),
            u8::from(at.month()),
            at.day()
        );
        let raw = shape.raw(transport, &date, at.unix_timestamp());
        let id = jmap.import(&raw, &received_at);
        Self {
            jmap,
            id,
            shape,
            sent_at: at.unix_timestamp(),
        }
    }

    fn on_server(&self) -> bool {
        let found = self.jmap.call(
            "Email/get",
            json!({ "ids": [self.id], "properties": ["id"] }),
        );
        found["list"]
            .as_array()
            .is_some_and(|list| !list.is_empty())
    }

    /// Whether the device lists it in alice's Sent folder.
    fn listed(&self, app: &MailcalApp) -> bool {
        let list = app.mailbox_list();
        list.rows.into_iter().any(|row| match row {
            SnapshotRow::Flat { row } => match self.shape {
                Shape::Long => row.subject.contains(MARKER),
                Shape::Short => row.subject == SHORT_SUBJECT,
            },
            _ => false,
        })
    }
}

impl Drop for Planted {
    fn drop(&mut self) {
        self.jmap.destroy(core::slice::from_ref(&self.id));
    }
}

fn learning(app: &MailcalApp, intent: StyleLearningIntent) {
    app.dispatch(Intent::Ai {
        intent: AiIntent::WritingStyle {
            intent: WritingStyleIntent::Learning { intent },
        },
    });
}

/// A model to send to, so Next reads; the review sends nothing, so nothing answers there.
fn an_endpoint(app: &MailcalApp) {
    app.dispatch(Intent::Ai {
        intent: AiIntent::Endpoint {
            intent: AiEndpointIntent::Save {
                address: "http://127.0.0.1:28434/v1".to_owned(),
                model: "local-model".to_owned(),
                declared: Some(JurisdictionClass::EuNative),
                key: String::new(),
            },
        },
    });
    eventually("the endpoint saved", || {
        app.writing_style_page()
            .is_some_and(|page| page.learn_blocked.is_none())
            .then_some(())
    });
}

fn show_sent(app: &MailcalApp) {
    let (account, key) = eventually("alice's Sent folder", || {
        app.mailbox_list()
            .account_folders
            .into_iter()
            .find_map(|tree| {
                let sent = tree
                    .folders
                    .into_iter()
                    .find(|f| matches!(f.role, Some(FolderRole::Sent)))?;
                Some((tree.account_id, sent.key))
            })
    });
    app.dispatch(Intent::SetViewMode {
        mode: ViewMode::Flat,
    });
    app.dispatch(Intent::SelectFolder { account, key });
    eventually("alice's sent mail listed", || {
        (!app.mailbox_list().rows.is_empty()).then_some(())
    });
}

/// The review of all alice's sent mail, once its read has settled, with whether it says the
/// older mail could not be fetched.
fn review_everything(app: &MailcalApp) -> (StyleLearningReport, bool) {
    learning(app, StyleLearningIntent::Open);
    learning(
        app,
        StyleLearningIntent::SelectRange {
            range: StyleLearnRange::Everything,
        },
    );
    learning(app, StyleLearningIntent::Next);
    eventually("the review's read", || {
        match app.writing_style_learning()?.stage {
            StyleLearningStage::Review {
                state: StyleLearningReview::Ready { report },
                older_not_fetched,
                ..
            } => Some((report, older_not_fetched)),
            StyleLearningStage::Review {
                state: StyleLearningReview::NothingUsable { report },
                ..
            } => panic!("nothing usable: {report:?}"),
            StyleLearningStage::Failed { failure, .. } => panic!("the review failed: {failure:?}"),
            _ => None,
        }
    })
}

fn the_review_finds_and_lets_go_of_older_sent_mail(app: &Arc<MailcalApp>, planted: &Planted) {
    an_endpoint(app);
    show_sent(app);
    assert!(
        !planted.listed(app),
        "the planted message is outside the account's window"
    );

    let (report, older_not_fetched) = review_everything(app);
    assert!(!older_not_fetched, "the older mail was fetched");
    assert_eq!(
        report.found,
        planted.jmap.sent_count(),
        "every sent message is found, the one sent {DAYS_AGO} days ago included: {report:?}"
    );
    assert_eq!(report.not_loaded, 0, "every body was read: {report:?}");
    match planted.shape {
        Shape::Long => assert_eq!(
            report.oldest,
            Some(planted.sent_at),
            "the message sent {DAYS_AGO} days ago is usable: {report:?}"
        ),
        Shape::Short => assert!(
            report.oldest > Some(planted.sent_at),
            "under forty words of its own, the short message is not usable: {report:?}"
        ),
    }
    assert!(report.newest >= report.oldest, "{report:?}");

    // The list reads the store's rows whatever their date, so a message the device still held
    // would be listed in Sent.
    eventually("the older message removed from the device", || {
        (!planted.listed(app)).then_some(())
    });
    assert!(
        planted.on_server(),
        "the server keeps what the device let go"
    );
}

#[test]
fn older_sent_mail_is_fetched_found_and_let_go_over_jmap_and_imap() {
    let Ok(http) = std::env::var("STALWART_HTTP_ADDR") else {
        eprintln!("skipping older-sent live test: STALWART_HTTP_ADDR unset");
        return;
    };
    for shape in [Shape::Long, Shape::Short] {
        let planted = Planted::plant(&http, "jmap", shape);
        let (_dir, app) = app("older-sent-jmap", RecordingStore::default());
        let config = mailcal_bindings::jmap_account_config_toml(JmapSetup {
            email: LOGIN.to_owned(),
            server_url: Some(format!("http://{http}")),
            password: PASSWORD.to_owned(),
        })
        .expect("a JMAP config");
        app.add_account(config).expect("alice connects");
        the_review_finds_and_lets_go_of_older_sent_mail(&app, &planted);
    }

    let (Ok(_), Ok(imap)) = (
        std::env::var("MAILCAL_EXTRA_CA"),
        std::env::var("MAILCAL_HARNESS_IMAP"),
    ) else {
        eprintln!("skipping the IMAP half of the older-sent live test: MAILCAL_HARNESS_IMAP unset");
        return;
    };
    let (host, port) = imap.rsplit_once(':').expect("host:port");
    for shape in [Shape::Long, Shape::Short] {
        let planted = Planted::plant(&http, "imap", shape);
        let (_dir, app) = app("older-sent-imap", RecordingStore::default());
        let config = format!(
            "[imap]\naddr = \"127.0.0.1:{port}\"\nserver_name = \"{host}\"\n\
             username = \"{LOGIN}\"\npassword = \"{PASSWORD}\"\n\
             [smtp]\naddr = \"127.0.0.1:12587\"\nserver_name = \"{host}\"\nsecurity = \"starttls\"\n"
        );
        app.add_account(config).expect("alice connects");
        the_review_finds_and_lets_go_of_older_sent_mail(&app, &planted);
    }
}
