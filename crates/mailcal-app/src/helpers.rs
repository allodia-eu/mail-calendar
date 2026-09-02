//! Small pure helpers the [`crate::App`] runtime uses to mint outgoing identifiers
//! and normalise reply/forward subjects. Split out of `lib.rs` to keep it under the
//! 500-line limit. A production host would mint UUIDs instead of clock-derived ids.

/// The wall clock, as the engine's UTC type: the `DTSTAMP` a written or answered event carries.
///
/// Engine time types deliberately cannot read the system clock (so expansion stays
/// deterministic), so the host supplies it. Reported as an error rather than silently stamped,
/// since a write with a wrong `DTSTAMP` is a write another client may ignore, and on an iTIP
/// `REPLY` it is worse than ignored: `DTSTAMP` is what orders two answers to one revision, so a
/// wrong one can have an organiser keep the *earlier* answer.
pub(crate) fn now_utc() -> Result<engine_api::UtcDateTime, String> {
    let now = time::OffsetDateTime::now_utc();
    engine_api::UtcDateTime::new(
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
    .map_err(|err| format!("cannot read the clock: {err}"))
}

/// Nanoseconds since the Unix epoch from the wall clock (0 if the clock is before it).
fn wall_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos())
}

/// A unique-enough `Message-ID` for an outgoing draft, from the wall clock. The outbox
/// keys on it for idempotency; a production host would mint a UUID.
pub(crate) fn generated_message_id() -> String {
    format!("{}@allodia.local", wall_nanos())
}

/// A unique `Content-ID` for an inline part the core mints itself: a signature's embedded image
/// on the way out (`crate::mail_compose_signature`). Unlike a quoted original's images, which
/// keep the sender's ids, a signature's bytes have never been a MIME part before, so there is no
/// id to preserve and one is minted here. `seq` distinguishes several images in the same
/// signature, which the clock alone would not (they are rewritten inside one tick).
pub(crate) fn generated_content_id(seq: usize) -> String {
    format!("sig{seq}.{}@allodia.local", wall_nanos())
}

/// A unique, **URL-safe** calendar event uid (no `@` or other chars a CalDAV server
/// would percent-encode in the resource href, so the PUT href round-trips). A
/// process-wide counter guards against same-tick collisions; a production host would
/// mint a UUID.
pub(crate) fn generated_uid() -> String {
    unique("evt")
}

/// A unique, URL-safe **contact** uid, on the same terms as [`generated_uid`]: it becomes the
/// CardDAV resource href, so nothing in it may be percent-encoded.
pub(crate) fn generated_contact_uid() -> String {
    unique("ctc")
}

/// The shared minting: the wall clock, plus a process-wide counter for same-tick collisions.
fn unique(prefix: &str) -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{prefix}-{}-{seq}", wall_nanos())
}

/// The reply subject for `original`: `Re: <subject>`, not doubled if it already is one.
pub(crate) fn reply_subject(original: Option<&str>) -> String {
    prefixed_subject("Re:", original)
}

/// The forward subject for `original`: `Fwd: <subject>`, not doubled.
pub(crate) fn forward_subject(original: Option<&str>) -> String {
    prefixed_subject("Fwd:", original)
}

/// Prefixes `original` with `prefix` (e.g. `Re:`/`Fwd:`) unless it already starts with
/// it (case-insensitively), so subjects don't accrete `Re: Re:`.
fn prefixed_subject(prefix: &str, original: Option<&str>) -> String {
    let subject = original.unwrap_or_default().trim();
    if subject
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
    {
        subject.to_owned()
    } else if subject.is_empty() {
        prefix.to_owned()
    } else {
        format!("{prefix} {subject}")
    }
}

/// A unique-per-call idempotency key for a mail edit. It must be unique per edit intent
/// (the outbox dedups by key across every op state, so two distinct edits; mark-read
/// then mark-unread, or edits of two messages; must not collide). A process-wide
/// monotonic counter guarantees uniqueness even when two calls land in the same clock
/// tick (the wall clock alone is not granular enough to rely on); a production host
/// would mint a UUID.
pub(crate) fn generated_idempotency() -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("edit:{}:{seq}", wall_nanos())
}
