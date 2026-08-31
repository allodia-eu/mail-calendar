//! A fake [`MailBackend`] the protocol and tool tests drive.
//!
//! It exists because the backend is a *port*, not a concrete app: the whole reason this crate
//! reaches the running app through a trait is that the trait can be faked in a page of code. So
//! these tests exercise the wire format, the tool surface and the policy controls without an
//! engine, a store, or a tokio runtime shaped like a real one, and the behaviour the fake
//! stands in for (ordering, scope, write semantics) is tested where it lives, in `mailcal-app`.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use mailcal_app::{MailActionError, MessageDetail, MessagePage, SendActionError};
use mailcal_viewmodel::{AccountRow, FlatRow, FolderRole, FolderRow, SearchHorizon};

use crate::backend::{AgentDraft, ComposerError, MailBackend};

/// What the fake was asked to do, so a test can assert on the calls rather than only the answers.
#[derive(Debug, Default)]
pub(crate) struct Recorder {
    /// Every write, as `("verb", account, key)`.
    pub(crate) writes: Vec<(String, String, String)>,
    /// Every draft opened.
    pub(crate) drafts: Vec<AgentDraft>,
    /// Every message sent.
    pub(crate) sends: Vec<Vec<String>>,
}

/// A backend over a fixed, tiny mailbox.
#[derive(Debug)]
pub(crate) struct FakeBackend {
    pub(crate) recorder: Arc<Mutex<Recorder>>,
    /// Addresses the recipient index knows.
    pub(crate) known: Vec<String>,
    /// When set, every write fails with this error.
    pub(crate) write_error: Option<MailActionError>,
    /// Whether a host composer is registered.
    pub(crate) has_composer: bool,
}

impl Default for FakeBackend {
    fn default() -> Self {
        Self {
            recorder: Arc::new(Mutex::new(Recorder::default())),
            known: vec!["colleague@known.example".to_owned()],
            write_error: None,
            has_composer: true,
        }
    }
}

impl FakeBackend {
    /// The fake, plus a handle on what it was asked to do.
    pub(crate) fn new() -> (Arc<Self>, Arc<Mutex<Recorder>>) {
        let backend = Self::default();
        let recorder = Arc::clone(&backend.recorder);
        (Arc::new(backend), recorder)
    }

    fn record(&self, verb: &str, account: &str, key: &str) -> Result<(), MailActionError> {
        if let Some(error) = self.write_error {
            return Err(error);
        }
        self.recorder.lock().unwrap().writes.push((
            verb.to_owned(),
            account.to_owned(),
            key.to_owned(),
        ));
        Ok(())
    }
}

/// The one message the fake mailbox holds.
fn row() -> FlatRow {
    FlatRow {
        account: "work".to_owned(),
        key: "m1".to_owned(),
        subject: "Quarterly report".to_owned(),
        from: "Ada".to_owned(),
        from_address: "ada@example.test".to_owned(),
        avatar: mailcal_viewmodel::avatar::resolve("Ada", "ada@example.test", None),
        date: "2026-07-27T09:00:00Z".to_owned(),
        unread: true,
        flagged: false,
        has_attachment: false,
        preview: "the numbers are in".to_owned(),
    }
}

#[async_trait]
impl MailBackend for FakeBackend {
    async fn accounts(&self) -> Vec<AccountRow> {
        vec![
            AccountRow {
                id: "work".to_owned(),
                email: "me@work.example".to_owned(),
                expanded: true,
            },
            AccountRow {
                id: "private".to_owned(),
                email: "me@private.example".to_owned(),
                expanded: true,
            },
        ]
    }

    async fn folders(&self, _account: &str) -> Vec<FolderRow> {
        vec![FolderRow {
            key: "inbox".to_owned(),
            name: "Inbox".to_owned(),
            role: Some(FolderRole::Inbox),
            unread: 3,
        }]
    }

    async fn folder_page(
        &self,
        _account: &str,
        _folder: Option<&str>,
        _unread_only: bool,
        offset: usize,
        _limit: usize,
    ) -> MessagePage {
        MessagePage {
            rows: vec![row()],
            total: 1,
            offset,
            windowed: false,
            horizon: Some(SearchHorizon::AllTime),
        }
    }

    async fn search(
        &self,
        _query: &str,
        _account: Option<&str>,
        _folder: Option<&str>,
        offset: usize,
        _limit: usize,
    ) -> MessagePage {
        MessagePage {
            rows: vec![
                row(),
                FlatRow {
                    account: "private".to_owned(),
                    key: "p1".to_owned(),
                    ..row()
                },
            ],
            total: 2,
            offset,
            windowed: true,
            horizon: Some(SearchHorizon::Months(3)),
        }
    }

    async fn message(&self, account: &str, key: &str) -> Option<MessageDetail> {
        (key == "m1").then(|| MessageDetail {
            account: account.to_owned(),
            key: key.to_owned(),
            subject: "Quarterly report".to_owned(),
            from: "Ada <ada@known.example>".to_owned(),
            to: "me@work.example".to_owned(),
            date: "2026-07-27T09:00:00Z".to_owned(),
            unread: true,
            body_text: "The numbers are in.".to_owned(),
            ..MessageDetail::default()
        })
    }

    async fn mark_read(
        &self,
        account: &str,
        key: &str,
        _read: bool,
    ) -> Result<(), MailActionError> {
        self.record("mark_read", account, key)
    }

    async fn set_flagged(
        &self,
        account: &str,
        key: &str,
        _flagged: bool,
    ) -> Result<(), MailActionError> {
        self.record("set_flagged", account, key)
    }

    async fn archive(&self, account: &str, key: &str) -> Result<(), MailActionError> {
        self.record("archive", account, key)
    }

    async fn trash(&self, account: &str, key: &str) -> Result<(), MailActionError> {
        self.record("trash", account, key)
    }

    async fn spam(&self, account: &str, key: &str) -> Result<(), MailActionError> {
        self.record("spam", account, key)
    }

    async fn send_plain(
        &self,
        _account: Option<&str>,
        to: &[String],
        _cc: &[String],
        _bcc: &[String],
        _subject: String,
        _body: String,
    ) -> Result<(), SendActionError> {
        self.recorder.lock().unwrap().sends.push(to.to_vec());
        Ok(())
    }

    async fn known_recipients(&self, query: &str) -> Vec<String> {
        let query = query.trim().to_ascii_lowercase();
        self.known
            .iter()
            .filter(|address| address.eq_ignore_ascii_case(&query))
            .cloned()
            .collect()
    }

    fn open_composer(&self, draft: AgentDraft) -> Result<(), ComposerError> {
        if !self.has_composer {
            return Err(ComposerError::NoHostComposer);
        }
        self.recorder.lock().unwrap().drafts.push(draft);
        Ok(())
    }
}
