//! How a test CONFIGURES the [`FakeProvider`] (the fixture DSL, in one file) as distinct from
//! what it then answers as a provider, which is `provider.rs`.
//!
//! One `impl` block of constructors and `with_*` variants: each says what a scenario needs (an
//! account's mailboxes, how much is unread, a folder that refuses, a send that fails) without a
//! test having to assemble the state by hand.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use engine_core::{
    ids::MailboxId,
    mail::{Mailbox, MailboxRole, Message},
};
use engine_provider::{
    Capabilities, MailEdit, MessageReport, ReportControls, ReportEvidence, ReportVerdicts,
};
use tokio::sync::Notify;

use super::{super::message, FakeProvider, StreamGate};

impl FakeProvider {
    pub(crate) fn new() -> Self {
        Self::with(vec![
            message("m1", "a", "Quarterly report"),
            message("m2", "a", "Lunch plans"),
        ])
    }

    /// A provider whose inbox holds `messages` (all in the role-Inbox mailbox `a`), reporting
    /// `unread` of them unread: the count a real server returns for the folder, independent of
    /// which messages the sync happens to have pulled.
    pub(crate) fn with_unread(messages: Vec<Message>, unread: u32) -> Self {
        let mut provider = Self::with(messages);
        for mailbox in &mut provider.mailboxes {
            mailbox.unread_count = Some(unread);
        }
        provider
    }

    /// Holds every [`Provider::edit_mail`] open until `gate` releases it, so a test can issue a
    /// second write to one message inside the first's round trip.
    pub(crate) fn gating_edits(mut self, gate: &super::EditGate) -> Self {
        self.edit_gate = Some(gate.share());
        self
    }

    /// A provider whose inbox holds `messages` (all in the role-Inbox mailbox `a`).
    pub(crate) fn with(messages: Vec<Message>) -> Self {
        let mut inbox = Mailbox::new(MailboxId::try_from("a").unwrap(), "Inbox");
        inbox.role = Some(MailboxRole::Inbox);
        Self {
            caps: Capabilities::none()
                .with_mail()
                .with_mail_writes()
                // All three verdicts, like IMAP, JMAP and Graph. `without_reporting` models
                // an adapter that has none, and `without_phishing_report` models Gmail.
                .with_mail_report(ReportControls {
                    verdicts: ReportVerdicts::all(),
                    evidence: ReportEvidence::Convention,
                })
                .with_message_source(),
            mailboxes: vec![inbox],
            messages,
            edits: Arc::new(Mutex::new(Vec::new())),
            reports: Arc::new(Mutex::new(Vec::new())),
            fail: Arc::new(AtomicBool::new(false)),
            refuse_edits: Arc::new(AtomicBool::new(false)),
            syncs: Arc::new(AtomicUsize::new(0)),
            email_mailbox: None,
            concurrent_fetches: 1,
            peak_in_flight: Arc::new(Mutex::new((0, 0))),
            sources_per_request: 1,
            batches: Arc::new(Mutex::new(Vec::new())),
            source_override: None,
            stream_gate: None,
            late: Arc::new(Mutex::new(Vec::new())),
            refuses_signin: false,
            source_fetches: Arc::new(AtomicUsize::new(0)),
            source_failures: Vec::new(),
            throttled_fetches: Arc::new(AtomicUsize::new(0)),
            throttle_wait_secs: None,
            tree: None,
            edit_gate: None,
        }
    }

    /// A provider that yields its first email chunk, then waits until `finish` is notified.
    pub(crate) fn blocking(messages: Vec<Message>) -> (Self, Arc<Notify>, Arc<Notify>) {
        let after_commit = Arc::new(Notify::new());
        let finish = Arc::new(Notify::new());
        let mut provider = Self::with(messages);
        provider.stream_gate = Some(StreamGate {
            after_commit: Arc::clone(&after_commit),
            finish: Arc::clone(&finish),
        });
        (provider, after_commit, finish)
    }

    /// A provider whose inbox holds `messages` and that advertises IMAP `IDLE` push, so
    /// the sync-settings snapshot offers (and defaults to) "receive as they arrive".
    pub(crate) fn with_idle(messages: Vec<Message>) -> Self {
        let mut provider = Self::with(messages);
        provider.caps = provider.caps.with_idle();
        provider
    }

    /// A provider whose inbox holds `messages` plus an Archive-role mailbox `archive`, so an
    /// archive action can resolve a destination folder by role.
    pub(crate) fn with_archive(messages: Vec<Message>) -> Self {
        let mut inbox = Mailbox::new(MailboxId::try_from("a").unwrap(), "Inbox");
        inbox.role = Some(MailboxRole::Inbox);
        let mut archive = Mailbox::new(MailboxId::try_from("archive").unwrap(), "Archive");
        archive.role = Some(MailboxRole::Archive);
        Self {
            mailboxes: vec![inbox, archive],
            ..Self::with(messages)
        }
    }

    /// A provider whose inbox holds `messages` plus an "Archieven" folder the server did **not**
    /// tag with `\Archive` (no role); exercises the conventional-name fallback for archive.
    pub(crate) fn with_named_archive(messages: Vec<Message>) -> Self {
        let mut inbox = Mailbox::new(MailboxId::try_from("a").unwrap(), "Inbox");
        inbox.role = Some(MailboxRole::Inbox);
        let archive = Mailbox::new(MailboxId::try_from("archief").unwrap(), "Archieven");
        Self {
            mailboxes: vec![inbox, archive],
            ..Self::with(messages)
        }
    }

    /// A provider shaped like **Gmail**: an Inbox, an `\All` "All Mail" mailbox, and **no
    /// Archive at all**: no `\Archive` role and no conventionally-named folder. Gmail has no
    /// Archive place; archiving there is the *absence* of the Inbox label, and the engine
    /// surfaces the resulting home as its synthetic All-Mail mailbox. Exercises the `\All`
    /// fallback, without which archive is a silent no-op on every Gmail account.
    pub(crate) fn with_all_mail(messages: Vec<Message>) -> Self {
        let mut inbox = Mailbox::new(MailboxId::try_from("INBOX").unwrap(), "Inbox");
        inbox.role = Some(MailboxRole::Inbox);
        let mut all_mail = Mailbox::new(MailboxId::try_from("ALL_MAIL").unwrap(), "All Mail");
        all_mail.role = Some(MailboxRole::All);
        Self {
            mailboxes: vec![inbox, all_mail],
            ..Self::with(messages)
        }
    }

    /// A provider whose store holds `messages` across an Inbox, a Sent folder, and an Archive
    /// folder (all role-tagged): for exercising the thread archive, which moves the received
    /// side to Archive but must leave the Sent copies in Sent.
    pub(crate) fn with_sent_and_archive(messages: Vec<Message>) -> Self {
        let mut inbox = Mailbox::new(MailboxId::try_from("a").unwrap(), "Inbox");
        inbox.role = Some(MailboxRole::Inbox);
        let mut sent = Mailbox::new(MailboxId::try_from("sent").unwrap(), "Sent");
        sent.role = Some(MailboxRole::Sent);
        let mut archive = Mailbox::new(MailboxId::try_from("archive").unwrap(), "Archive");
        archive.role = Some(MailboxRole::Archive);
        Self {
            mailboxes: vec![inbox, sent, archive],
            ..Self::with(messages)
        }
    }

    /// A provider whose store holds `messages` across an Inbox, an Archive, and a **Trash**
    /// folder (all role-tagged): for the search-scope tests, where which folder a hit sits in
    /// is the whole point.
    pub(crate) fn with_trash(messages: Vec<Message>) -> Self {
        let mut inbox = Mailbox::new(MailboxId::try_from("a").unwrap(), "Inbox");
        inbox.role = Some(MailboxRole::Inbox);
        let mut archive = Mailbox::new(MailboxId::try_from("archive").unwrap(), "Archive");
        archive.role = Some(MailboxRole::Archive);
        let mut trash = Mailbox::new(MailboxId::try_from("trash").unwrap(), "Trash");
        trash.role = Some(MailboxRole::Trash);
        Self {
            mailboxes: vec![inbox, archive, trash],
            ..Self::with(messages)
        }
    }

    /// A folder-bound provider the on-demand connector hands back: its email scope is a
    /// distinct per-mailbox scope (so syncing it never tombstones the inbox's scope), and
    /// it carries no folder list (the list is already synced).
    pub(crate) fn folder(mailbox_key: &str, messages: Vec<Message>) -> Self {
        Self {
            mailboxes: Vec::new(),
            email_mailbox: Some(MailboxId::try_from(mailbox_key).unwrap()),
            ..Self::with(messages)
        }
    }

    /// A shared handle to the edits this provider receives.
    pub(crate) fn edits(&self) -> Arc<Mutex<Vec<MailEdit>>> {
        Arc::clone(&self.edits)
    }

    /// A shared handle to the reports this provider receives.
    pub(crate) fn reports(&self) -> Arc<Mutex<Vec<MessageReport>>> {
        Arc::clone(&self.reports)
    }

    /// Adds a role-`Junk` folder, so a spam report has somewhere to file the message.
    pub(crate) fn with_junk_folder(mut self) -> Self {
        let mut junk = Mailbox::new(MailboxId::try_from("junk").unwrap(), "Junk");
        junk.role = Some(MailboxRole::Junk);
        self.mailboxes.push(junk);
        self
    }

    /// Lets the account's folders be changed: the folders so far become a tree every
    /// folder-list sync reads whole and every folder change is applied to.
    pub(crate) fn with_folder_tree(mut self) -> Self {
        self.caps = self.caps.with_mailbox_writes();
        self.tree = Some(Arc::new(Mutex::new(self.mailboxes.clone())));
        self
    }

    /// The changeable tree, so a test can change it as another client would, or read it as
    /// the server holds it.
    pub(crate) fn folder_tree(&self) -> Arc<Mutex<Vec<Mailbox>>> {
        Arc::clone(self.tree.as_ref().expect("built with_folder_tree"))
    }

    /// An IMAP-shaped Inbox provider: bound to the folder `a`, and listing it as the Inbox
    /// beside a role-less folder per key in `others`, the way the provider an IMAP account's
    /// folder list is read from reports every folder while syncing only its own.
    pub(crate) fn imap_inbox(messages: Vec<Message>, others: &[&str]) -> Self {
        let mut inbox = Mailbox::new(MailboxId::try_from("a").unwrap(), "Inbox");
        inbox.role = Some(MailboxRole::Inbox);
        let listed = others
            .iter()
            .map(|key| Mailbox::new(MailboxId::try_from(*key).unwrap(), *key));
        let mut provider = Self::folder("a", messages);
        provider.mailboxes = std::iter::once(inbox).chain(listed).collect();
        provider
    }

    /// Lists the inbox under the id `id`, as an IMAP server's spelling of it once was.
    pub(crate) fn listing_inbox_as(mut self, id: &str) -> Self {
        let inbox = self
            .mailboxes
            .iter_mut()
            .find(|mailbox| mailbox.role == Some(MailboxRole::Inbox))
            .expect("an inbox");
        inbox.id = MailboxId::try_from(id).unwrap();
        self
    }

    /// Lists the folder `key` as a level of the tree that holds no mail, as an IMAP server lists
    /// a `\Noselect` folder.
    pub(crate) fn with_container(mut self, key: &str) -> Self {
        let folder = self
            .mailboxes
            .iter_mut()
            .find(|mailbox| mailbox.id.as_str() == key)
            .expect("a listed folder");
        folder.selectable = false;
        self
    }

    /// Files two folders inside the provider's existing Archive: `Clients`, and `Acme` inside
    /// that. Two levels because one proves only that a row was indented once, where the rule a
    /// pane has to hold is that a row is on screen while **every** folder above it is open.
    pub(crate) fn with_nested_folders(mut self) -> Self {
        let mut clients = Mailbox::new(
            MailboxId::try_from("archive/clients").unwrap(),
            // The folder's own name, which is what every adapter reports: the path to it is
            // the id, and the pane's indent is what says where it sits.
            "Clients",
        );
        clients.parent = Some(MailboxId::try_from("archive").unwrap());
        let mut acme = Mailbox::new(MailboxId::try_from("archive/clients/acme").unwrap(), "Acme");
        acme.parent = Some(clients.id.clone());
        self.mailboxes.push(clients);
        self.mailboxes.push(acme);
        self
    }

    /// An adapter that cannot report at all: the dev fixtures and the showcase engine, and
    /// any transport added later without a junk verb. The core files the message itself.
    pub(crate) fn without_reporting(mut self) -> Self {
        self.caps = Capabilities::none()
            .with_mail()
            .with_mail_writes()
            .with_message_source();
        self
    }

    /// An adapter with no phishing verdict; Gmail, whose label set has no phishing member.
    pub(crate) fn without_phishing_report(mut self) -> Self {
        self.caps = self.caps.with_mail_report(ReportControls {
            verdicts: ReportVerdicts::without_phishing(),
            evidence: ReportEvidence::Convention,
        });
        self
    }

    /// A shared handle to this provider's failure switch, so a test can knock it offline (and
    /// back) between operations; e.g. to prove a warmed body still opens from the cache with
    /// the provider down.
    /// Reports `n` as the transport's fetch width, so a test can drive the body warm as an
    /// HTTP adapter (overlapping fetches) rather than a single-socket one.
    pub(crate) fn with_concurrent_fetches(mut self, n: usize) -> Self {
        self.concurrent_fetches = n;
        self
    }

    /// Reports that one batch request carries up to `n` sources, as a `UID FETCH` set does.
    pub(crate) fn with_sources_per_request(mut self, n: usize) -> Self {
        self.sources_per_request = n;
        self
    }

    /// The keys of every batch the app asked this provider for, in order.
    pub(crate) fn batches(&self) -> Arc<Mutex<Vec<Vec<String>>>> {
        Arc::clone(&self.batches)
    }

    /// The shared counter recording the most source fetches ever in flight at once.
    pub(crate) fn in_flight_peak(&self) -> Arc<Mutex<(usize, usize)>> {
        Arc::clone(&self.peak_in_flight)
    }

    pub(crate) fn failure_switch(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.fail)
    }

    /// A shared handle that, when on, makes the server refuse every edit **for good**: the
    /// outcome an outbox does not retry, as opposed to [`failure_switch`](Self::failure_switch)'s
    /// unreachable server, whose edits stay queued.
    pub(crate) fn refusal_switch(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.refuse_edits)
    }

    /// A shared handle to this provider's sync counter; how many times the app has streamed
    /// email from it.
    pub(crate) fn syncs(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.syncs)
    }

    /// Refuses the next `count` source fetches as throttled, naming `wait_secs` or no wait.
    pub(crate) fn throttling_fetches(mut self, count: usize, wait_secs: Option<u64>) -> Self {
        self.throttled_fetches = Arc::new(AtomicUsize::new(count));
        self.throttle_wait_secs = wait_secs;
        self
    }

    /// A shared handle to this provider's source-fetch counter; how many times it has been
    /// asked for a message's raw bytes, cache misses and failures alike.
    pub(crate) fn source_fetches(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.source_fetches)
    }

    /// Serves `raw` as the message source for every fetch, instead of the default hostile
    /// HTML body: so a test can open a message with a specific MIME shape.
    pub(crate) fn with_source(mut self, raw: &[u8]) -> Self {
        self.source_override = Some(raw.to_vec());
        self
    }

    /// Makes [`Provider::fetch_message_source`] fail for these message keys, so a test can
    /// model bodies the provider persistently cannot serve.
    pub(crate) fn with_failing_sources<I: IntoIterator<Item = String>>(mut self, keys: I) -> Self {
        self.source_failures = keys.into_iter().collect();
        self
    }

    /// A handle on the mail this provider delivers on its **next** cursored sync. Pushing a
    /// message here between two refreshes models a reply arriving after the mailbox was already
    /// synced and threaded.
    pub(crate) fn late_delivery(&self) -> Arc<Mutex<Vec<Message>>> {
        Arc::clone(&self.late)
    }

    /// A provider whose every sync fails with a retryable transport error: an account whose
    /// server can't be reached, so a refresh badges it unreachable.
    pub(crate) fn failing() -> Self {
        let provider = Self::new();
        provider.fail.store(true, Ordering::SeqCst);
        provider
    }

    /// Makes this provider's syncs fail the way a server that **refuses the credential** does; an
    /// authentication class, which the app reads as an expired sign-in rather than an outage. A
    /// server may answer this for a credential that is in fact valid, so a test can place a refused
    /// scope beside a working one on the same account.
    pub(crate) fn refusing_signin(mut self) -> Self {
        self.refuses_signin = true;
        self
    }
}
