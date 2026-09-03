//! Rich-compose mail operations: new-message / reply / reply-all / forward sends, plus the
//! reply-recipient suggestion that pre-fills a reply's editable To/Cc. A second `impl App`
//! block (like `reading`/`calendar_ops`) so `mail_ops.rs` stays under the 500-line limit; it
//! reuses the send + account helpers that live in `mail_ops.rs` (`send_draft`,
//! `account_identity`, `compose_account`, `fail_send`, `find_message_in`).

use std::collections::{HashMap, HashSet};

use engine_api::{
    AccountId, ContentIdHeader, Draft, DraftAttachment, EmailAddress, InlinePart, Message,
    MessageIdHeader, Provider,
};
use mailcal_composer::{
    ComposerDocument, DraftBlobHandle, OutputAttachment, render as render_composer,
};

use crate::{
    App,
    helpers::{forward_subject, generated_message_id, reply_subject},
    mail_compose_quote::{
        quotes_reference_inline_images, reattach_quote_cids, sanitize_quote_bodies,
    },
    protocol::{ComposerBlob, RecipientSuggestion},
    reference::MessageRef,
};

impl<P: Provider> App<P> {
    /// Renders a shared composer document, resolves host blob bytes, and submits the
    /// resulting rich draft through the durable outbox. `from` is the account the user picked in
    /// the composer's From dropdown; `None` derives it ([`App::compose_account`]: the selected
    /// account, else the default send account, else the first). The `to`/`cc`/`bcc` fields are
    /// the host's comma-separated recipient lists (see [`parse_addresses`]); Bcc recipients are
    /// delivered but hidden by the engine.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn submit_rich_mail(
        &self,
        from: Option<AccountId>,
        to: String,
        cc: String,
        bcc: String,
        subject: String,
        document: ComposerDocument,
        blobs: Vec<ComposerBlob>,
    ) {
        let account = match from {
            Some(account) => account,
            // No accounts configured at all, nothing to send from, and nothing to report.
            None => match self.compose_account().await {
                Some(account) => account,
                None => return,
            },
        };
        let Some(identity) = self.identity_or_fail(&account).await else {
            return;
        };
        let to = parse_addresses(&to);
        let cc = parse_addresses(&cc);
        let bcc = parse_addresses(&bcc);
        if to.is_empty() && cc.is_empty() && bcc.is_empty() {
            // No resolvable recipient; never send a recipient-less draft. The command surface
            // enforces this for every caller (the future MCP/AI adapters too), not just the
            // clients' Send-button gating.
            self.fail_send().await;
            return;
        }
        // A brand-new message has no quoted original, so no inline parts to re-attach.
        let Some(draft) = rich_draft(&identity, to, cc, bcc, subject, document, blobs, &[]) else {
            self.fail_send().await;
            return;
        };
        self.send_draft(&account, &draft).await;
    }

    /// Replies to `message` with a rich composer `document`, by default **from the account that
    /// received it** (the reference's account); `from` overrides that with the account the user
    /// picked in the composer's From dropdown, so a reply can go out from a different mailbox.
    /// Uses the host-supplied `to`/`cc`/`bcc` recipients (the host pre-fills these from
    /// [`App::reply_recipients`] and the user may edit them), takes the `subject` the user left
    /// in the composer (falling back to a derived `Re:` when the caller has no subject field),
    /// derives the `In-Reply-To`/`References` chain from the stored original so the reply
    /// threads, renders the rich draft, then sends through the sending account's outbox. A no-op
    /// if the original can't be resolved; a render/blob failure surfaces as a failed send.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn submit_rich_reply(
        &self,
        message: MessageRef,
        from: Option<AccountId>,
        to: String,
        cc: String,
        bcc: String,
        subject: Option<String>,
        document: ComposerDocument,
        blobs: Vec<ComposerBlob>,
    ) {
        let Some(original) = self.find_message_in(&message).await else {
            return;
        };
        // The composer's Subject field is editable on a reply, so what the user left there wins;
        // the derived `Re:` is the fallback for a caller that has no such field to read.
        let subject =
            subject.unwrap_or_else(|| reply_subject(original.envelope.subject.as_deref()));
        // The original always lives in `message.account`, only the *sending* account can differ.
        let account = from.unwrap_or_else(|| message.account.clone());
        let Some(identity) = self.identity_or_fail(&account).await else {
            return;
        };
        let to = parse_addresses(&to);
        let cc = parse_addresses(&cc);
        let bcc = parse_addresses(&bcc);
        if to.is_empty() && cc.is_empty() && bcc.is_empty() {
            // No resolvable recipient; never send a recipient-less reply.
            self.fail_send().await;
            return;
        }
        // Re-attach the quoted original's inline images as `cid:` parts on the way out (see
        // `original_inline_parts`); best-effort, so a fetch failure just leaves them as `data:`.
        // Skip the fetch entirely when no quote carries an inline image: the common reply.
        let inline_parts = if quotes_reference_inline_images(&document) {
            self.original_inline_parts(&message, &original).await
        } else {
            Vec::new()
        };
        let Some(mut draft) = rich_draft(
            &identity,
            to,
            cc,
            bcc,
            subject,
            document,
            blobs,
            &inline_parts,
        ) else {
            self.fail_send().await;
            return;
        };
        // Thread the reply: In-Reply-To the original, References = its chain + itself.
        if let Some(parent) = original.envelope.message_id.first() {
            let mut references = original.envelope.references.clone();
            references.push(parent.clone());
            draft = draft.in_reply_to(parent.clone(), references);
        }
        self.send_draft(&account, &draft).await;
    }

    /// Forwards `message` with a rich composer `document` to the host-supplied `to`/`cc`/`bcc`
    /// recipients, under the `subject` the user left in the composer (a derived `Fwd:` when the
    /// caller has no subject field). Sends from the reference's account by
    /// default; `from` overrides that with the account the user picked in the composer's From
    /// dropdown. A no-op if the original can't be resolved; a render/blob-resolution failure
    /// surfaces as a failed send.
    ///
    /// A forward carries the original's `References` chain but **no** `In-Reply-To`: it
    /// continues the conversation without answering a message. That is what puts the sent
    /// copy on the thread it came from; without it, every forward you send is a new
    /// one-message conversation sitting beside the discussion it belongs to.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn submit_rich_forward(
        &self,
        message: MessageRef,
        from: Option<AccountId>,
        to: String,
        cc: String,
        bcc: String,
        subject: Option<String>,
        document: ComposerDocument,
        blobs: Vec<ComposerBlob>,
    ) {
        let Some(original) = self.find_message_in(&message).await else {
            return;
        };
        let subject =
            subject.unwrap_or_else(|| forward_subject(original.envelope.subject.as_deref()));
        // The original always lives in `message.account`, only the *sending* account can differ.
        let account = from.unwrap_or_else(|| message.account.clone());
        let Some(identity) = self.identity_or_fail(&account).await else {
            return;
        };
        let to = parse_addresses(&to);
        let cc = parse_addresses(&cc);
        let bcc = parse_addresses(&bcc);
        if to.is_empty() && cc.is_empty() && bcc.is_empty() {
            // No resolvable recipient; never send a recipient-less draft.
            self.fail_send().await;
            return;
        }
        let inline_parts = if quotes_reference_inline_images(&document) {
            self.original_inline_parts(&message, &original).await
        } else {
            Vec::new()
        };
        let Some(mut draft) = rich_draft(
            &identity,
            to,
            cc,
            bcc,
            subject,
            document,
            blobs,
            &inline_parts,
        ) else {
            self.fail_send().await;
            return;
        };
        // Thread the forward: References = the original's chain + the original itself.
        if let Some(parent) = original.envelope.message_id.first() {
            let mut references = original.envelope.references.clone();
            references.push(parent.clone());
            draft = draft.with_references(references);
        }
        self.send_draft(&account, &draft).await;
    }

    /// Computes the suggested recipients for a reply (or reply-all) to `message`, for a
    /// host to pre-fill the composer's editable `To`/`Cc` fields before the user sends.
    ///
    /// The `To` always leads with the original's `Reply-To` (else `From`). A **reply-all**
    /// then **preserves the original structure**: the original's other `To` recipients stay
    /// in `To`, and its `Cc` recipients stay in `Cc`; matching Outlook/Thunderbird; a plain
    /// reply leaves `Cc` empty. The user's own identity is removed from both, and addresses
    /// are de-duplicated case-insensitively (an address already in `To` never repeats in
    /// `Cc`). Both fields are empty when the original isn't in the account's synced set.
    /// `Bcc` is never derived (it is always the user's own addition), so it is not suggested.
    pub async fn reply_recipients(
        &self,
        message: MessageRef,
        reply_all: bool,
    ) -> RecipientSuggestion {
        let Some(original) = self.find_message_in(&message).await else {
            return RecipientSuggestion::default();
        };
        let mut seen: HashSet<String> = HashSet::new();
        // The To leads with ALL the primary recipients: the original's Reply-To if it has any,
        // else its From. They go in unconditionally (a reply must have a recipient) even when
        // the sender is the user themselves (e.g. replying to a message in their own Sent
        // folder), where self-exclusion would otherwise leave the To empty and unsendable.
        let primary = if original.envelope.reply_to.is_empty() {
            &original.envelope.from
        } else {
            &original.envelope.reply_to
        };
        let mut to: Vec<EmailAddress> = Vec::new();
        for addr in primary {
            push_unique(&mut seen, addr, &mut to);
        }
        // Beyond the primary, a reply never targets the user themselves: exclude their own
        // identity from the reply-all expansion below.
        if let Some(own) = self.account_identity(&message.account).await {
            seen.insert(own.email.to_ascii_lowercase());
        }
        let mut cc: Vec<EmailAddress> = Vec::new();
        if reply_all {
            // Keep the original layout: its other To recipients join To, its Cc stays Cc.
            for addr in &original.envelope.to {
                push_unique(&mut seen, addr, &mut to);
            }
            for addr in &original.envelope.cc {
                push_unique(&mut seen, addr, &mut cc);
            }
        }
        RecipientSuggestion {
            to: join_emails(&to),
            cc: join_emails(&cc),
        }
    }

    /// Fetches the inline (`cid:`) parts of the message a reply/forward quotes, so the
    /// quoted body's resolved `data:` images can be re-attached as `cid:` parts on send
    /// (see [`reattach_quote_cids`]). Reuses the raw blob the reading-view body fetch
    /// already cached ([`engine_api::Engine::message_inline_parts`]). Best-effort: no
    /// account/provider or a fetch error yields an empty list, so the quote simply keeps
    /// its `data:` images rather than failing the send.
    async fn original_inline_parts(
        &self,
        message: &MessageRef,
        original: &Message,
    ) -> Vec<InlinePart> {
        let Some(acct) = self.account_handle(&message.account).await else {
            return Vec::new();
        };
        let Some(provider) = acct.providers.first() else {
            return Vec::new();
        };
        self.engine
            .message_inline_parts(provider, &message.account, original)
            .await
            .unwrap_or_default()
    }
}

/// Renders a composer document into a rich (HTML + plain-text + attachments) draft from
/// `identity` to the `to`/`cc`/`bcc` recipients, resolving inline/regular attachment bytes
/// from the host `blobs`. The base builder shared by new-message, reply, and forward; reply
/// layers its `In-Reply-To`/`References` threading onto the returned draft.
#[allow(clippy::too_many_arguments)]
fn rich_draft(
    identity: &EmailAddress,
    to: Vec<EmailAddress>,
    cc: Vec<EmailAddress>,
    bcc: Vec<EmailAddress>,
    subject: String,
    mut document: ComposerDocument,
    blobs: Vec<ComposerBlob>,
    inline_parts: &[InlinePart],
) -> Option<Draft> {
    // A quoted original is HTML the editor round-tripped back to us; untrusted. Re-sanitize
    // every quote body to the inert subset before it is rendered into the outgoing message, so
    // an edited (or injected) quote can never carry script/handlers into a sent draft. This is
    // the composer-security gate for quoted content; it runs in the shared core, so it holds on
    // every platform regardless of what a client's editor emits (see `docs/composer-security.md`).
    sanitize_quote_bodies(&mut document);
    // The signature block is the other body the composer emits verbatim, and it comes back
    // through the same untrusted editor: so it passes the same gate (`mail_compose_signature`).
    crate::mail_compose_signature::sanitize_signature_bodies(&mut document);
    // After re-sanitising, turn each quoted-original inline `data:` image back into a `cid:`
    // reference to its original part; what Outlook/Thunderbird produce, and what an Outlook
    // reader renders; collecting those parts to attach below. A quote with no such image, or a
    // non-reply (empty `inline_parts`), yields nothing.
    let quoted_inline = reattach_quote_cids(&mut document, inline_parts);
    // The signature's images take the same route to `cid:`, but from the opposite starting point:
    // they were never MIME parts, so their ids are minted rather than preserved.
    let signature_inline = crate::mail_compose_signature::reattach_signature_cids(&mut document);
    let output = render_composer(&document).ok()?;
    // A picture the editor captured itself (a paste, or the "show it in the message" answer on a
    // drop) has no host blob to resolve: its bytes ride in the document as a `data:` URI, keyed by
    // the same attachment id the manifest names.
    let data_urls: HashMap<&mailcal_composer::AttachmentId, &str> = document
        .attachments
        .iter()
        .filter_map(|attachment| {
            attachment
                .data_url
                .as_deref()
                .map(|url| (&attachment.id, url))
        })
        .collect();
    let message_id = MessageIdHeader::new(generated_message_id()).ok()?;
    let mut blob_map = blob_map(blobs);
    let mut draft = Draft::new(message_id, identity.clone(), to, subject, output.plain_text)
        .with_html_body(output.html)
        .with_cc(cc)
        .with_bcc(bcc);

    for attachment in output.inline_attachments {
        draft = draft.with_attachment(engine_attachment(
            attachment,
            &mut blob_map,
            &data_urls,
            true,
        )?);
    }
    for attachment in output.attachments {
        draft = draft.with_attachment(engine_attachment(
            attachment,
            &mut blob_map,
            &data_urls,
            false,
        )?);
    }
    // The re-attached quoted-original inline images carry their own bytes and Content-ID (there
    // is no host blob to resolve), so they are added straight to the draft. The signature's
    // rewritten images arrive the same way, with the ids minted for them.
    for attachment in quoted_inline.into_iter().chain(signature_inline) {
        draft = draft.with_attachment(attachment);
    }
    Some(draft)
}

/// Splits a recipient field's comma-separated text into addresses, trimming whitespace and
/// dropping empties. These fields carry bare addresses (no display-name-with-comma parsing),
/// matching the plain composer; it is the inverse of [`join_emails`].
fn parse_addresses(field: &str) -> Vec<EmailAddress> {
    field
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(EmailAddress::new)
        .collect()
}

/// Appends `addr` to `into` unless its (lowercased) email is already in `seen`; the
/// case-insensitive de-dup that keeps a reply-all from listing anyone twice (or in both `To`
/// and `Cc`), while preserving first-seen order.
fn push_unique(seen: &mut HashSet<String>, addr: &EmailAddress, into: &mut Vec<EmailAddress>) {
    if seen.insert(addr.email.to_ascii_lowercase()) {
        into.push(addr.clone());
    }
}

/// Joins addresses into the comma-separated text a host shows in a recipient field; the
/// bare email of each (the composer's plain-text fields round-trip bare addresses).
fn join_emails(addresses: &[EmailAddress]) -> String {
    addresses
        .iter()
        .map(|addr| addr.email.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn blob_map(blobs: Vec<ComposerBlob>) -> HashMap<DraftBlobHandle, Vec<u8>> {
    blobs
        .into_iter()
        .map(|blob| (blob.handle, blob.bytes))
        .collect()
}

/// Resolves one manifest entry into an engine attachment, taking its bytes from the host blob it
/// names or, for a picture the document carried itself, by decoding its `data:` URI.
///
/// The decode is the narrow one the signature rewrite uses: base64 `image/*` only, so a document
/// can never turn arbitrary bytes into a part. An entry that names neither a resolvable blob nor a
/// decodable image fails the send rather than putting an empty part on the wire.
fn engine_attachment(
    attachment: OutputAttachment,
    blobs: &mut HashMap<DraftBlobHandle, Vec<u8>>,
    data_urls: &HashMap<&mailcal_composer::AttachmentId, &str>,
    inline: bool,
) -> Option<DraftAttachment> {
    let OutputAttachment {
        id,
        blob,
        file_name,
        media_type,
        size,
        cid,
    } = attachment;
    // The decoded URI's own media type wins over the manifest's: it describes the bytes actually
    // being attached, and it is the one the narrow image check passed.
    let (media_type, bytes) = match blob {
        Some(blob) => (media_type, blobs.remove(&blob)?),
        None => crate::mail_compose_signature::decode_data_image(data_urls.get(&id).copied()?)?,
    };
    if let Some(expected) = size
        && expected != bytes.len() as u64
    {
        return None;
    }
    if inline {
        let cid = ContentIdHeader::new(cid?.as_str()).ok()?;
        Some(DraftAttachment::inline(file_name, media_type, cid, bytes))
    } else {
        Some(DraftAttachment::attachment(file_name, media_type, bytes))
    }
}
