//! The reading-view read (`App::open_message`): fetch one message's body, sanitise its
//! HTML, and publish a [`ReadingSnapshot`] for the host. A second `impl App` block (like
//! `calendar_ops`) so `lib.rs` stays small.

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use engine_api::{
    AccountId, AttachmentPartId, EmailAddress, Message, MessageAttachment, Provider, SystemKeyword,
};
use mailcal_viewmodel::{
    AttachmentRow, Avatar, ReadingSnapshot,
    avatar::{self},
};

use crate::{App, html, reference::MessageRef};

/// How long a cold reading-open waits for the opened message's account to finish dialing its
/// mail provider before giving up, polled in small steps. A cold open (e.g. tapped from a
/// notification) can beat the async connect that follows boot; sized to cover that connect
/// (observed a few seconds over several accounts) without hanging on a genuine load failure.
const OPEN_DIAL_WAIT: Duration = Duration::from_secs(8);
const OPEN_DIAL_POLL: Duration = Duration::from_millis(400);

/// How long an open may run before the reading view is told it is still waiting.
///
/// Under this, the open publishes once (the body) and no loading indicator is ever shown.
/// A cached body resolves in a few milliseconds, so announcing every open put a spinner on
/// screen and took it away again within the same eyeblink; moving between messages read as
/// flickering rather than as fast. 500 ms is long enough that anything shorter reads as
/// instant, and short enough that a real wait is never silent.
const READING_PENDING_AFTER: Duration = Duration::from_millis(500);

/// Rising id for one reading-open, so two overlapping opens can be told apart in the log and a
/// retried one stays recognisable across its attempts.
static OPEN_SEQ: AtomicU64 = AtomicU64::new(0);

/// Times a reading-open step by step, at `debug`.
///
/// The reading view sits on its spinner until [`App::open_message`] publishes, and every step
/// between is an `await`: a store read, the accounts lock, a provider round-trip, the inline-image
/// and attachment reads. When an open does not come back, "which one of those is it in" is the only
/// question worth asking, and one elapsed figure for the whole open cannot answer it.
///
/// Durations, byte counts and a synthetic id only: no key, no address, no subject
/// (`docs/logging.md`).
struct OpenTrace {
    id: u64,
    started: Instant,
    lap: Instant,
}

impl OpenTrace {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            id: OPEN_SEQ.fetch_add(1, Ordering::Relaxed),
            started: now,
            lap: now,
        }
    }

    /// Logs how long the step just finished took, and starts the next lap.
    fn step(&mut self, what: &str) {
        log::debug!(
            "read[{}]: {what} in {}ms",
            self.id,
            self.lap.elapsed().as_millis()
        );
        self.lap = Instant::now();
    }

    /// Logs an outcome against the whole open rather than the current lap.
    fn total(&self, what: &str) {
        log::debug!(
            "read[{}]: {what} after {}ms",
            self.id,
            self.started.elapsed().as_millis()
        );
    }
}

impl<P: Provider> App<P> {
    /// The body for `message`: one fetch, plus the bounded retry a cold open needs.
    ///
    /// Split out of [`Self::open_message`] so the *whole* wait sits under one timeout. A first
    /// fetch that fails fast and then retries for the account to dial is still a user waiting on
    /// a blank pane; timing only the first attempt would leave that wait unannounced for as
    /// long as [`OPEN_DIAL_WAIT`].
    async fn resolve_reading(
        &self,
        message: &MessageRef,
        trace: &mut OpenTrace,
    ) -> ReadingSnapshot {
        let mut snapshot = self.fetch_reading_traced(message.clone(), trace).await;
        // A cold open (e.g. tapped from a notification) can beat the account's mail provider,
        // which dials asynchronously a moment after boot: the body fetch then fails for lack of a
        // provider, not because the message is unavailable. Keep waiting and retry while the
        // account has no connected mail provider yet; bounded, so a genuinely unavailable
        // message (or a still-offline account) still surfaces its error, never hangs.
        let mut waited = Duration::ZERO;
        while snapshot.load_error
            && waited < OPEN_DIAL_WAIT
            && !self.account_has_mail_provider(&message.account).await
        {
            trace.total("still waiting for the account's mail provider to connect");
            tokio::time::sleep(OPEN_DIAL_POLL).await;
            waited += OPEN_DIAL_POLL;
            trace.lap = Instant::now();
            snapshot = self.fetch_reading_traced(message.clone(), trace).await;
        }
        snapshot
    }

    /// Fetches the body of `message` (its account + provider key bound together),
    /// sanitises its HTML, stores the reading snapshot, and signals [`Surface::Reading`].
    ///
    /// The raw RFC 5322 source is fetched from the provider on the first open and served
    /// from the store's blob cache afterwards ([`engine_api::Engine::message_body`]). The
    /// provider's `fetch_message_source` selects the message's own mailbox by key, so the
    /// first mail provider serves any folder (the routing mark-read/reply already rely on).
    /// The host always gets a snapshot for the key it opened; a failed fetch is flagged via
    /// [`ReadingSnapshot::load_error`] so it is distinguishable from a body-less message.
    pub(crate) async fn open_message(&self, message: MessageRef) {
        // One trace across the retries: a reading-open that never comes back is the same open to
        // the user however many attempts it took, and separate ids would hide that.
        let mut trace = OpenTrace::new();
        let snapshot = {
            // The body, and (only if it takes long enough to be worth saying so) a snapshot
            // that announces the wait first. Pinned and re-awaited rather than raced against a
            // spawned timer: it is the same future either way, so the announcement cannot
            // outlive the open that earned it or land after a newer one has published.
            let mut resolve = core::pin::pin!(self.resolve_reading(&message, &mut trace));
            // `as_mut` reborrows, so the timeout does not consume the future: on expiry the
            // work carries on from where it got to rather than starting again.
            if let Ok(snapshot) =
                tokio::time::timeout(READING_PENDING_AFTER, resolve.as_mut()).await
            {
                snapshot
            } else {
                self.reading.publish(ReadingSnapshot {
                    key: message.key.as_str().to_owned(),
                    pending: true,
                    ..ReadingSnapshot::default()
                });
                resolve.await
            }
        };
        // Mark the message as read on the server once the body has loaded successfully.
        // Skip when the body was unavailable (load_error): the user never saw the content;
        // or when it is already marked Seen. Publish the snapshot first so the reading view
        // opens immediately; the mark-read settles in the background.
        let load_failed = snapshot.load_error;
        self.reading.publish(snapshot);
        // The wait ends here, and only here; everything above it is what the user waits on.
        trace.total(if load_failed {
            "published a load error"
        } else {
            "published the message"
        });
        if !load_failed {
            let is_read = self
                .find_message_in(&message)
                .await
                .is_some_and(|m| m.has_system_keyword(SystemKeyword::Seen));
            if !is_read {
                let _ = self.mark_read(message, true).await;
            }
        }
    }

    /// Whether `account` currently has at least one connected mail provider. A freshly booted
    /// account dials asynchronously and stays a provider-less placeholder until then, so this
    /// lets a cold reading-open tell "still connecting" apart from a genuine load failure.
    async fn account_has_mail_provider(&self, account: &AccountId) -> bool {
        self.account_handle(account)
            .await
            .is_some_and(|acct| !acct.providers.is_empty())
    }

    /// Resolves and fetches the body for `message` **within its account**. Returns a
    /// `load_error` snapshot when the body could not be fetched: the message isn't in that
    /// account's synced set, the account has no mail provider, or a provider/network error
    /// (which also covers a provider that can't fetch sources, e.g. one without
    /// `message_source` support): as distinct from a body-less message.
    ///
    /// **This is the non-mutating read path.** [`App::open_message`] is only the wrapper that
    /// stores the snapshot, signals the surface, and marks the message read on the server; the
    /// agent adapter's `query_message` calls *this* instead, so an assistant reading a message
    /// does not silently mark it read in the user's mailbox. Do not copy `open_message`'s body.
    /// The reading header's avatar, for the same sender the header names.
    ///
    /// The photo comes from the map the list already filled, so opening a message shows the
    /// face the row showed rather than falling back to the monogram.
    pub(crate) fn sender_avatar(&self, address: Option<&EmailAddress>) -> Avatar {
        address.map_or_else(Avatar::default, |address| {
            avatar::resolve(
                address.name.as_deref().unwrap_or_default(),
                &address.email,
                self.resolved_photo(&address.email),
            )
        })
    }

    pub(crate) async fn fetch_reading(&self, message: MessageRef) -> ReadingSnapshot {
        self.fetch_reading_traced(message, &mut OpenTrace::new())
            .await
    }

    /// [`fetch_reading`](Self::fetch_reading), timing each `await` into `trace`. Separate so
    /// [`open_message`](Self::open_message) can keep one trace across its retries.
    async fn fetch_reading_traced(
        &self,
        message: MessageRef,
        trace: &mut OpenTrace,
    ) -> ReadingSnapshot {
        let key = message.key.as_str().to_owned();
        let load_error = |key: String| ReadingSnapshot {
            key,
            load_error: true,
            ..Default::default()
        };
        let Some(original) = self.find_message_in(&message).await else {
            trace.step("gave up: the message is not in that account's synced set");
            return load_error(key);
        };
        trace.step("located the message");
        // Clone the account handle, then fetch with the read guard released: the source
        // fetch is a network round-trip on the first open and must not hold the lock.
        let body = {
            let Some(acct) = self.account_handle(&message.account).await else {
                trace.step("gave up: no such account");
                return load_error(key);
            };
            let Some(provider) = acct.providers.first() else {
                trace.step("gave up: the account has no connected mail provider");
                return load_error(key);
            };
            trace.step("took the account handle");
            self.engine
                .message_body(provider, &message.account, &original)
                .await
        };
        let Ok(body) = body else {
            trace.step("gave up: the body could not be fetched");
            return load_error(key);
        };
        // The one step that can be a network round-trip, and the first suspect whenever an open
        // does not come back. Its size is here because a very large body explains a slow open that
        // nothing else would.
        trace.step(&format!(
            "read the body ({} html / {} plain bytes)",
            body.html().map_or(0, str::len),
            body.plain().map_or(0, str::len)
        ));
        // `html` is the engine's raw (unsanitised) HTML; sanitise before it leaves the core
        // (`html::sanitize`), flagging remote images so the host can gate them; `plain` is
        // the text view. When the body references inline `cid:` images, resolve them to
        // self-contained `data:` URIs so they render under the existing CSP; inline images
        // are part of the message (local), not remote content, so this is not gated.
        let (html, has_remote_images) = match body.html() {
            Some(raw) => {
                let sanitized = html::sanitize(raw);
                let html = if sanitized.has_cid_references {
                    self.resolve_inline_images(&message, &original, sanitized.html)
                        .await
                } else {
                    sanitized.html
                };
                (Some(html), sanitized.has_remote_images)
            }
            None => (None, false),
        };
        // Sanitise *and* any inline-image resolution: the latter reuses the raw blob the body
        // fetch just cached, but it is still an await, so it is named rather than hidden here.
        trace.step("prepared the html");
        let attachments = self.message_attachments(&message, &original).await;
        trace.step("read the attachments");
        // The invitation card, when this message carries an iTIP object that warrants one. Reads
        // the same cached raw source the body fetch just populated, so it costs no extra
        // round-trip; `None` for ordinary mail and for anything the RSVP gate rejects.
        let invitation = self.invitation_card(&message, &original).await;
        trace.step("built the invitation card");
        ReadingSnapshot {
            key,
            // This snapshot IS the answer, so it never announces a wait; `pending` is only ever
            // set by the separate one `open_message` publishes while still working.
            pending: false,
            // The sender line, shown in the reading header as the full `Name <email>` (the list
            // row shows just the name). `from` is a list, but a message has one author, take the
            // first, matching the sender the list row summarises.
            from: original
                .envelope
                .from
                .first()
                .map_or_else(String::new, format_address),
            // The header draws the avatar beside the sender, so it names the same person the
            // list row did, and must therefore draw the *same* face. Built without the photo
            // and it replaces the row's when the snapshot lands: the list shows a photograph
            // and opening the message shows a monogram.
            avatar: self.sender_avatar(original.envelope.from.first()),
            // The recipient headers, for the reading view. `bcc` is only ever populated on the
            // sender's own Sent/Drafts copy (whose stored message carries a Bcc header), so
            // the sender sees whom they Bcc'd, while a received message shows none.
            to: format_addresses(&original.envelope.to),
            cc: format_addresses(&original.envelope.cc),
            bcc: format_addresses(&original.envelope.bcc),
            html,
            plain: body.plain().map(str::to_owned),
            has_remote_images,
            load_error: false,
            attachments,
            invitation,
        }
    }

    /// Resolves the inline `cid:` image references in `sanitized` to self-contained `data:`
    /// URIs, fetching the message's inline parts from its account's provider
    /// ([`engine_api::Engine::message_inline_parts`], which reuses the raw blob the body
    /// fetch already cached). **Best-effort**: any failure: no account/provider, or a
    /// fetch/store error, returns the sanitised HTML unchanged, leaving the references as
    /// inert broken images rather than failing the open. Called only when the body actually
    /// references `cid:`, so a message with no inline images never reaches the parts fetch.
    async fn resolve_inline_images(
        &self,
        message: &MessageRef,
        original: &Message,
        sanitized: String,
    ) -> String {
        let Some(acct) = self.account_handle(&message.account).await else {
            return sanitized;
        };
        let Some(provider) = acct.providers.first() else {
            return sanitized;
        };
        match self
            .engine
            .message_inline_parts(provider, &message.account, original)
            .await
        {
            Ok(parts) => html::inline_cid_images(&sanitized, &parts),
            Err(_) => sanitized,
        }
    }

    /// Lists downloadable attachments for the open message. Best-effort: a provider/cache
    /// error leaves the attachment strip empty while still showing the message body.
    async fn message_attachments(
        &self,
        message: &MessageRef,
        original: &Message,
    ) -> Vec<AttachmentRow> {
        let Some(acct) = self.account_handle(&message.account).await else {
            return Vec::new();
        };
        let Some(provider) = acct.providers.first() else {
            return Vec::new();
        };
        self.engine
            .message_attachments(provider, &message.account, original)
            .await
            .map(|attachments| attachments.iter().map(attachment_row).collect())
            .unwrap_or_default()
    }

    /// Saves one message attachment to `destination_path`.
    ///
    /// The destination is a host-selected filesystem path (save panel, app-cache staging
    /// file, etc.). The bytes do not cross FFI; Rust decodes the selected MIME part from the
    /// cached raw source and writes the file directly.
    ///
    /// # Errors
    ///
    /// Returns a plain error string for malformed references, missing accounts/providers,
    /// missing attachment ids, provider/cache failures, or filesystem write failures.
    pub async fn save_attachment(
        &self,
        message: MessageRef,
        attachment_id: u32,
        destination_path: &str,
    ) -> Result<(), String> {
        let Some(original) = self.find_message_in(&message).await else {
            return Err("message is not available".to_owned());
        };
        let Some(acct) = self.account_handle(&message.account).await else {
            return Err("account is not connected".to_owned());
        };
        let Some(provider) = acct.providers.first() else {
            return Err("mail provider is not connected".to_owned());
        };
        let content = self
            .engine
            .message_attachment(
                provider,
                &message.account,
                &original,
                AttachmentPartId::new(attachment_id),
            )
            .await
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "attachment is not available".to_owned())?;
        std::fs::write(destination_path, content.bytes()).map_err(|err| err.to_string())
    }
}

fn attachment_row(attachment: &MessageAttachment) -> AttachmentRow {
    AttachmentRow {
        id: attachment.id().as_u32(),
        file_name: attachment.file_name().to_owned(),
        media_type: attachment.media_type().to_owned(),
        size: attachment.size(),
    }
}

/// Formats an address list for display: each as `Name <email>` (when a display name is
/// present) or bare `email`, comma-joined. Empty for an empty list.
fn format_addresses(addresses: &[EmailAddress]) -> String {
    addresses
        .iter()
        .map(format_address)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Formats one address as `Name <email>` when it has a non-blank display name, else `email`.
fn format_address(address: &EmailAddress) -> String {
    match &address.name {
        Some(name) if !name.trim().is_empty() => format!("{name} <{}>", address.email),
        _ => address.email.clone(),
    }
}
