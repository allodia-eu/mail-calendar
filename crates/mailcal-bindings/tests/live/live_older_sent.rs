//! Gated live check of learning from sent mail older than the account's sync window
//! (`docs/ai.md` rule 11), over JMAP and over IMAP: a message alice sent 240 days ago, before her
//! three-month window, is fetched for the review, read by it, and removed from the device once the
//! review has read it, while the server keeps it.
//!
//! The message is planted in alice's `Sent Items` over JMAP (`Email/import`, whose `receivedAt`
//! is the IMAP `INTERNALDATE`) and destroyed again however the test ends. Nothing is sent.
//!
//! Skips unless `STALWART_HTTP_ADDR` is set; the IMAP half also needs `MAILCAL_HARNESS_IMAP` and
//! `MAILCAL_EXTRA_CA`. `scripts/dev/harness.sh test` sets all three. By hand:
//! ```sh
//! scripts/dev/harness.sh up
//! STALWART_HTTP_ADDR=127.0.0.1:28080 MAILCAL_HARNESS_IMAP=localhost:12993 \
//!   MAILCAL_EXTRA_CA=docker/stalwart/tls/harness-ca.pem \
//!   cargo test -p mailcal-bindings --test live -- live_older_sent:: --nocapture
//! ```

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use mailcal_bindings::{
    AiEndpointIntent, AiIntent, FolderRole, Intent, JmapSetup, JurisdictionClass, MailcalApp,
    SnapshotRow, StyleLearnRange, StyleLearningIntent, StyleLearningReport, StyleLearningReview,
    StyleLearningStage, ViewMode, WritingStyleIntent,
};
use serde_json::{Value, json};
use time::{OffsetDateTime, Time};

use crate::live_oauth::{LOGIN, PASSWORD, RecordingStore, app};

/// The token the planted message's subject carries, so a rerun can find an earlier run's residue.
const MARKER: &str = "older-sent-live";

/// How long ago the planted message was sent: before the account's three-month window.
const DAYS_AGO: i64 = 240;

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

/// alice's JMAP account on the harness, reached as a test reaches it, never through the core.
struct Jmap {
    http: reqwest::Client,
    base: String,
    account: String,
    runtime: tokio::runtime::Runtime,
}

impl Jmap {
    fn connect(addr: &str) -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("a runtime for the JMAP calls");
        let http = mailcal_oauth::discovery_client().expect("the shared HTTP client");
        let base = format!("http://{addr}");
        let session = Self::block_on(
            &runtime,
            http.get(format!("{base}/jmap/session"))
                .basic_auth(LOGIN, Some(PASSWORD))
                .send(),
        );
        let account = session["primaryAccounts"]["urn:ietf:params:jmap:mail"]
            .as_str()
            .expect("alice's mail account")
            .to_owned();
        Self {
            http,
            base,
            account,
            runtime,
        }
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "a test drives a runtime of its own from the test harness's thread, never a host's"
    )]
    fn block_on(
        runtime: &tokio::runtime::Runtime,
        request: impl Future<Output = reqwest::Result<reqwest::Response>>,
    ) -> Value {
        runtime.block_on(async {
            let response = request.await.expect("the harness answers");
            let status = response.status();
            let text = response.text().await.expect("a readable answer");
            assert!(
                status.is_success(),
                "the harness refused ({status}): {text}"
            );
            serde_json::from_str(&text).expect("the answer is JSON")
        })
    }

    /// The answer to one method call.
    fn call(&self, method: &str, arguments: Value) -> Value {
        let mut arguments = arguments;
        arguments["accountId"] = json!(self.account);
        let body = json!({
            "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
            "methodCalls": [[method, arguments, "0"]],
        });
        let answer = Self::block_on(
            &self.runtime,
            self.http
                .post(format!("{}/jmap/", self.base))
                .basic_auth(LOGIN, Some(PASSWORD))
                .header("Content-Type", "application/json")
                .body(body.to_string())
                .send(),
        );
        let [name, result, _] = &answer["methodResponses"][0]
            .as_array()
            .expect("one response")[..]
        else {
            panic!("a malformed response: {answer}");
        };
        assert_eq!(name, method, "the call failed: {result}");
        result.clone()
    }

    fn sent_mailbox(&self) -> String {
        let found = self.call("Mailbox/query", json!({ "filter": { "role": "sent" } }));
        found["ids"][0].as_str().expect("a Sent mailbox").to_owned()
    }

    /// The ids of the messages whose subject carries `MARKER`.
    fn planted(&self) -> Vec<String> {
        let found = self.call("Email/query", json!({ "filter": { "subject": MARKER } }));
        found["ids"]
            .as_array()
            .expect("ids")
            .iter()
            .filter_map(|id| id.as_str().map(str::to_owned))
            .collect()
    }

    fn destroy(&self, ids: &[String]) {
        if !ids.is_empty() {
            self.call("Email/set", json!({ "destroy": ids }));
        }
    }

    /// Files `raw` in alice's Sent mailbox as received at `received_at`, answering its id.
    fn import(&self, raw: &str, received_at: &str) -> String {
        let blob = Self::block_on(
            &self.runtime,
            self.http
                .post(format!("{}/jmap/upload/{}/", self.base, self.account))
                .basic_auth(LOGIN, Some(PASSWORD))
                .header("Content-Type", "message/rfc822")
                .body(raw.to_owned())
                .send(),
        );
        let imported = self.call(
            "Email/import",
            json!({ "emails": { "planted": {
                "blobId": blob["blobId"],
                "mailboxIds": { self.sent_mailbox(): true },
                "keywords": { "$seen": true },
                "receivedAt": received_at,
            }}}),
        );
        imported["created"]["planted"]["id"]
            .as_str()
            .unwrap_or_else(|| panic!("the message was not imported: {imported}"))
            .to_owned()
    }
}

/// The message planted for one run, destroyed on the server however the run ends.
struct Planted {
    jmap: Jmap,
    id: String,
    /// When it was sent, in seconds since the Unix epoch.
    sent_at: i64,
    subject: String,
}

impl Planted {
    fn plant(addr: &str, transport: &str) -> Self {
        let jmap = Jmap::connect(addr);
        jmap.destroy(&jmap.planted());
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
        let subject = format!("{MARKER} {transport} notes for the spring board meeting");
        let raw = format!(
            "From: Alice <alice@test.local>\r\nTo: Ruth Jansen <ruth@test.local>\r\n\
             Subject: {subject}\r\nDate: {date}\r\n\
             Message-ID: <{MARKER}-{transport}-{}@test.local>\r\nMIME-Version: 1.0\r\n\
             Content-Type: text/plain; charset=utf-8\r\n\r\n\
             Dear Ruth,\r\n\r\nThank you for sending the draft agenda so early. I have read it \
             through twice and I think the order works well, although I would move the budget \
             item before the break so that we are not rushed at the end of the afternoon.\r\n\r\n\
             Could you also ask the treasurer to bring the printed figures? Several members \
             asked for them last time.\r\n\r\nKind regards,\r\nAlice\r\n",
            at.unix_timestamp()
        );
        let id = jmap.import(&raw, &received_at);
        Self {
            jmap,
            id,
            sent_at: at.unix_timestamp(),
            subject,
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

/// The subjects of the rows the device lists in alice's Sent folder.
fn sent_subjects(app: &MailcalApp) -> Vec<String> {
    let list = app.mailbox_list();
    list.rows
        .into_iter()
        .filter_map(|row| match row {
            SnapshotRow::Flat { row } => Some(row.subject),
            _ => None,
        })
        .collect()
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
        (!sent_subjects(app).is_empty()).then_some(())
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

fn the_review_reads_and_lets_go_of_older_sent_mail(app: &Arc<MailcalApp>, planted: &Planted) {
    an_endpoint(app);
    show_sent(app);
    assert!(
        !sent_subjects(app).contains(&planted.subject),
        "the planted message is outside the account's window"
    );

    let (report, older_not_fetched) = review_everything(app);
    assert!(!older_not_fetched, "the older mail was fetched");
    assert_eq!(
        report.oldest,
        Some(planted.sent_at),
        "the review read the message sent {DAYS_AGO} days ago: {report:?}"
    );
    assert!(report.newest > report.oldest, "{report:?}");

    // The list reads the store's rows whatever their date, so a message the device still held
    // would be listed in Sent.
    eventually("the older message removed from the device", || {
        (!sent_subjects(app).contains(&planted.subject)).then_some(())
    });
    assert!(
        planted.on_server(),
        "the server keeps what the device let go"
    );
}

#[test]
fn older_sent_mail_is_fetched_read_and_let_go_over_jmap_and_imap() {
    let Ok(http) = std::env::var("STALWART_HTTP_ADDR") else {
        eprintln!("skipping older-sent live test: STALWART_HTTP_ADDR unset");
        return;
    };
    {
        let planted = Planted::plant(&http, "jmap");
        let (_dir, app) = app("older-sent-jmap", RecordingStore::default());
        let config = mailcal_bindings::jmap_account_config_toml(JmapSetup {
            email: LOGIN.to_owned(),
            server_url: Some(format!("http://{http}")),
            password: PASSWORD.to_owned(),
        })
        .expect("a JMAP config");
        app.add_account(config).expect("alice connects");
        the_review_reads_and_lets_go_of_older_sent_mail(&app, &planted);
    }

    let (Ok(_), Ok(imap)) = (
        std::env::var("MAILCAL_EXTRA_CA"),
        std::env::var("MAILCAL_HARNESS_IMAP"),
    ) else {
        eprintln!("skipping the IMAP half of the older-sent live test: MAILCAL_HARNESS_IMAP unset");
        return;
    };
    let planted = Planted::plant(&http, "imap");
    let (host, port) = imap.rsplit_once(':').expect("host:port");
    let (_dir, app) = app("older-sent-imap", RecordingStore::default());
    let config = format!(
        "[imap]\naddr = \"127.0.0.1:{port}\"\nserver_name = \"{host}\"\n\
         username = \"{LOGIN}\"\npassword = \"{PASSWORD}\"\n\
         [smtp]\naddr = \"127.0.0.1:12587\"\nserver_name = \"{host}\"\nsecurity = \"starttls\"\n"
    );
    app.add_account(config).expect("alice connects");
    the_review_reads_and_lets_go_of_older_sent_mail(&app, &planted);
}
