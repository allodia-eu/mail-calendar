//! The per-composition record: what the core knows about each composer that is open.

use std::{collections::HashMap, sync::Arc};

use engine_api::{AccountId, InlinePart, MessageIdHeader, PendingOpId, ProviderKey};

use crate::{CompositionId, DraftStatus};

/// What the core knows about one composition, for as long as its composer is open.
pub(super) struct Composition {
    /// The account whose Drafts folder holds it. Fixed by the first save: the stored copy
    /// lives in one account's folder, and moving it later would leave that copy behind.
    pub(super) account: AccountId,
    /// The header every save of this composition carries. See the module docs for why it is
    /// minted once rather than per save.
    pub(super) message_id: MessageIdHeader,
    /// What the server stores the draft under **now**, or `None` until a save has actually
    /// reached one. Handed to the next save as `replacing`.
    pub(super) key: Option<ProviderKey>,
    /// A digest of what the last save put on the server, so an unchanged draft costs no
    /// write. Compared only within this process, which is all
    /// [`DefaultHasher`] promises.
    pub(super) saved: Option<u64>,
    /// The queued save waiting for a network, if the last one did not get through.
    ///
    /// Held so the drain that eventually stores it can hand its key back here. Nothing else
    /// can: the op settles with nobody watching, and a settled op is not in the queue read.
    /// Without it a composer that stayed open through the outage would save again with
    /// `replacing` empty and leave a second draft on the server.
    pub(super) queued: Option<PendingOpId>,
    /// The message this one answers, for a composition reopened on a reply: every save and
    /// the send carry it, so the reply stays in its conversation.
    pub(super) threading: Option<Threading>,
    /// The pictures the message it was reopened on carried as parts. The quoted original
    /// shows them as `data:` URIs, and every save and the send turn those back into the parts
    /// they came from, as a reply's own quote does with its original's.
    pub(super) pictures: Arc<[InlinePart]>,
}

impl Composition {
    /// A composition with nothing saved yet.
    pub(super) fn new(account: AccountId, message_id: MessageIdHeader) -> Self {
        Self {
            account,
            message_id,
            key: None,
            saved: None,
            queued: None,
            threading: None,
            pictures: Arc::from([]),
        }
    }
}

/// The headers that place a reply in its conversation.
#[derive(Clone)]
pub(super) struct Threading {
    pub(super) in_reply_to: MessageIdHeader,
    pub(super) references: Vec<MessageIdHeader>,
}

/// The open compositions, keyed by the id their host minted.
#[derive(Default)]
pub(crate) struct DraftState {
    pub(super) open: HashMap<CompositionId, Composition>,
    /// How each composition's most recent save ended.
    ///
    /// Its own map rather than a field on [`Composition`], because a save can fail before
    /// there is anything to record it against: no account to save to, or nothing to render.
    ///
    /// Per composition, not one slot for the app, on the rule the reading windows follow
    /// (`docs/reading-window.md`): a desktop has several composers open and each is saving a
    /// different draft, so one slot would have the composer nobody touched announce that the
    /// one beside it had saved.
    pub(super) status: HashMap<CompositionId, DraftStatus>,
    /// Held for the length of a save, so the read of the stored key and the write of the new
    /// one cannot be split by another save.
    ///
    /// A composer has two triggers, the idle timer and the Save button, and nothing stops
    /// both firing; each intent is its own task. The second reads `replacing` before the
    /// first has recorded what it stored, so it supersedes nothing and the server is left
    /// holding two copies of the message still being written.
    ///
    /// **The engine already stops the two provider calls overlapping**: both saves of one
    /// composition share a resource key, and an op leases it. That is what makes the failure
    /// quiet rather than a visible error. It does nothing for the stale read, which happens
    /// here, before the engine is asked anything.
    pub(super) save: Arc<tokio::sync::Mutex<()>>,
}
