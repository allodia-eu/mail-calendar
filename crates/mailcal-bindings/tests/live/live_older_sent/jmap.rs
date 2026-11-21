//! alice's JMAP account on the harness, reached as a test reaches it, never through the core: to
//! plant a sent message, count her Sent mail and destroy what was planted.

use serde_json::{Value, json};

use crate::live_oauth::{LOGIN, PASSWORD};

pub(super) struct Jmap {
    http: reqwest::Client,
    base: String,
    account: String,
    runtime: tokio::runtime::Runtime,
}

impl Jmap {
    pub(super) fn connect(addr: &str) -> Self {
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
    pub(super) fn call(&self, method: &str, arguments: Value) -> Value {
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

    /// Every message in alice's Sent mailbox, with its subject and `Message-ID`.
    fn sent(&self) -> Vec<Value> {
        let found = self.call(
            "Email/query",
            json!({ "filter": { "inMailbox": self.sent_mailbox() } }),
        );
        let got = self.call(
            "Email/get",
            json!({ "ids": found["ids"], "properties": ["id", "subject", "messageId"] }),
        );
        got["list"].as_array().cloned().unwrap_or_default()
    }

    /// How many messages alice's Sent mailbox holds.
    pub(super) fn sent_count(&self) -> u32 {
        u32::try_from(self.sent().len()).expect("a count")
    }

    /// The ids of the Sent messages `planted` says a run planted, `planted` given the subject and
    /// the first `Message-ID`.
    pub(super) fn planted(&self, planted: impl Fn(&str, &str) -> bool) -> Vec<String> {
        self.sent()
            .into_iter()
            .filter(|email| {
                planted(
                    email["subject"].as_str().unwrap_or_default(),
                    email["messageId"][0].as_str().unwrap_or_default(),
                )
            })
            .filter_map(|email| email["id"].as_str().map(str::to_owned))
            .collect()
    }

    pub(super) fn destroy(&self, ids: &[String]) {
        if !ids.is_empty() {
            self.call("Email/set", json!({ "destroy": ids }));
        }
    }

    /// Files `raw` in alice's Sent mailbox, seen, as received at `received_at` (the IMAP
    /// `INTERNALDATE`), answering its id.
    pub(super) fn import(&self, raw: &str, received_at: &str) -> String {
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
