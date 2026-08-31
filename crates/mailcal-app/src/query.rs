//! Stateless request/response mail queries: the **read** side of an agent adapter.
//!
//! # Why these are not Intents
//!
//! Reads go through a different door from writes, and that asymmetry is the load-bearing
//! decision of the whole agent surface. Writes *should* be [`Intent`](crate::Intent)s: an
//! assistant's archive then happens in the user's own list, visibly, by the same mechanism their
//! swipe uses. Reads must not be, for two reasons that are easy to miss until a user watches
//! their screen move on its own:
//!
//! 1. **Every read-shaped intent moves the user's screen.** `Search`, `SelectFolder`, `ShowMore`
//!    and `SetViewMode` all end in a `rebuild_snapshot()` that republishes the mailbox list. An
//!    assistant answering "what's in my inbox?" would scroll and re-scope the window of a person
//!    who is reading something else.
//! 2. **`Intent::OpenMessage` marks the message read on the server** (`crate::reading`). "Read me
//!    that email" would silently clear the unread badge on the user's real mailbox; an
//!    irreversible, server-side side effect of a question.
//!
//! So a query answers a question and changes nothing: no snapshot is republished, no selection
//! moves, no keyword is written, and the shared message cache is not even warmed (a query reads
//! the store directly, so its paging depth cannot perturb what the UI has loaded). Two guarantee
//! tests in `tests_query.rs` exist solely to stop a later contributor collapsing this back into
//! one path.
//!
//! Reuse, however, is deliberate everywhere it is safe: ordering, scope vocabulary and folder
//! sorting all come from the same functions the UI uses, so an agent and a person are never
//! shown the same mailbox in two different orders.

use engine_api::{AccountId, Provider, SystemKeyword, UtcDateTime};
use mailcal_viewmodel::{AccountRow, FlatRow, FolderRow, SearchHorizon, sorted_folder_rows};

use crate::{App, reference::MessageRef};

mod list;
mod search;
mod text;

/// The deepest store read a query will drive, in messages.
///
/// Paging here is a **window, not a cursor** (see [`MessagePage::windowed`]), so `offset + limit`
/// decides how far back the read reaches. This caps that: a client asking for offset 900 000
/// must not turn into a full-mailbox deserialization.
const MAX_QUERY_WINDOW: usize = 5_000;

/// One page of message rows, in the same newest-first order the mailbox list uses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessagePage {
    /// The rows in this page; at most the requested `limit`, newest first.
    pub rows: Vec<FlatRow>,
    /// How many messages match in total, before `offset`/`limit`. For a folder listing this is
    /// the full in-scope count within the read window, for a search it is the number of hits
    /// the engine's ranked candidate set yielded.
    pub total: usize,
    /// The offset these rows start at, echoed back so a caller can page without bookkeeping.
    pub offset: usize,
    /// Whether the page was assembled from a **bounded newest-first window** rather than a true
    /// cursor over the whole mailbox.
    ///
    /// The engine offers `messages(account)`, `messages_windowed(account, limit)`,
    /// `messages_by_keys` and `thread_messages`; there is **no folder-scoped, offset-capable
    /// read**. So a page is cut from the account's newest-N slice, and a folder whose mail is
    /// all older than that slice is not reachable by raising `offset`. This flag exists so the
    /// fact travels with the data instead of living only in a doc comment: a caller can say
    /// "older mail exists that I cannot page to" rather than implying the mailbox ended.
    ///
    /// Tracked upstream as `email-calendar-sync-engine#83`, the same engine limitation
    /// `docs/search.md` already points at.
    pub windowed: bool,
    /// How far back this device holds mail for the accounts the page drew from: the sync
    /// depth, narrowest first (`docs/search.md`). `None` when it drew from no account.
    ///
    /// Separate from [`windowed`](Self::windowed), and both are needed: that one says older
    /// mail exists which paging cannot reach, this one says older mail was never downloaded at
    /// all. An assistant told neither reads an empty answer as "no such message".
    pub horizon: Option<SearchHorizon>,
}

/// One message in full: its headers, its flags, and its body **as plain text**.
///
/// The body is plain text *by construction*, not by policy. HTML is a strictly larger prompt
/// injection surface than text; hidden spans, white-on-white, CSS `content`, all of which
/// survive sanitisation because sanitisation is about script execution, not about what a
/// language model reads: so the conversion happens here, in the core, and an adapter over this
/// type structurally cannot emit HTML it was never given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessageDetail {
    /// The owning account's id.
    pub account: String,
    /// The message's provider key.
    pub key: String,
    /// The subject (empty if none).
    pub subject: String,
    /// The sender, as `Name <email>` or a bare address.
    pub from: String,
    /// The `To` recipients, comma-joined.
    pub to: String,
    /// The `Cc` recipients, comma-joined.
    pub cc: String,
    /// The `Bcc` recipients, comma-joined; populated only on the sender's own copy.
    pub bcc: String,
    /// The message's instant as a whole-second RFC 3339 `…Z` string (empty if unknown). The
    /// core is tzdata-free; a client localises it (`docs/timestamps.md`).
    pub date: String,
    /// Whether the message is unread. Reading it through this type does **not** change it.
    pub unread: bool,
    /// Whether the message is flagged.
    pub flagged: bool,
    /// The body as plain text: the message's own `text/plain` part when it has one, else the
    /// text extracted from its sanitised HTML. Empty when the message has no body at all.
    pub body_text: String,
    /// The names of the message's downloadable attachments. **Names only**: no bytes cross
    /// this boundary, so an assistant can say what is attached without an attacker being able
    /// to feed it a file's contents.
    pub attachment_names: Vec<String>,
    /// Whether the body could not be fetched (an offline account, a provider error), as
    /// distinct from a message that genuinely has no body.
    pub load_error: bool,
}

impl<P: Provider> App<P> {
    /// Every configured account (id + address).
    pub async fn query_accounts(&self) -> Vec<AccountRow> {
        self.account_rows().await
    }

    /// One account's folders, in the **same canonical order the sidebar shows**; special
    /// folders first in a fixed order, then the rest by name (`mailcal_viewmodel::folders`). An
    /// agent and a person therefore name the same folder the same way.
    pub async fn query_folders(&self, account: &AccountId) -> Vec<FolderRow> {
        sorted_folder_rows(&self.engine.mailboxes(account).await.unwrap_or_default())
    }

    /// One message in full, **without marking it read**.
    ///
    /// One line different from `App::open_message`: it calls `fetch_reading` (the pure fetch)
    /// rather than the wrapper that stores the snapshot, signals `Surface::Reading`, and writes
    /// `$seen` to the server. That single difference is why this has a regression test rather
    /// than a comment: the two call sites are three characters apart and the wrong one is
    /// silently destructive.
    pub async fn query_message(&self, message: &MessageRef) -> Option<MessageDetail> {
        let original = self.find_message_in(message).await?;
        let snapshot = self.fetch_reading(message.clone()).await;
        let body_text = match snapshot.plain.as_deref() {
            Some(plain) if !plain.trim().is_empty() => plain.to_owned(),
            // No text/plain part: extract text from the already-sanitised HTML. Never the HTML.
            _ => snapshot
                .html
                .as_deref()
                .map(text::to_plain)
                .unwrap_or_default(),
        };
        Some(MessageDetail {
            account: message.account.as_str().to_owned(),
            key: message.key.as_str().to_owned(),
            subject: original.envelope.subject.clone().unwrap_or_default(),
            from: snapshot.from,
            to: snapshot.to,
            cc: snapshot.cc,
            bcc: snapshot.bcc,
            date: original
                .received_at
                .or(original.sent_at)
                .map_or_else(String::new, format_instant),
            unread: !original.has_system_keyword(SystemKeyword::Seen),
            flagged: original.has_system_keyword(SystemKeyword::Flagged),
            body_text,
            attachment_names: snapshot
                .attachments
                .into_iter()
                .map(|attachment| attachment.file_name)
                .collect(),
            load_error: snapshot.load_error,
        })
    }
}

/// Formats a UTC instant as a whole-second `…Z` string, matching what the mailbox-list rows
/// carry so a client's date parser sees one shape from both surfaces.
fn format_instant(instant: UtcDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        instant.year(),
        instant.month(),
        instant.day(),
        instant.hour(),
        instant.minute(),
        instant.second(),
    )
}
