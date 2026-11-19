//! The per-composition record: what the core knows about each composer that is open.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use engine_api::{
    AccountId, Importance, InlinePart, MessageIdHeader, PendingOpId, Provider, ProviderKey,
};

use crate::{
    App, ComposeKind, CompositionId, DraftStatus, ai::ReplyDraftHeld, reference::MessageRef,
};

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
    /// The conversation the message belongs to, joined when the composer opened: a reply's or a
    /// forward's from its original, a reopened message's from its own headers. Every save and the
    /// send carry it, so the message stays in its conversation however it is left and resumed.
    pub(super) threading: Option<Threading>,
    /// The parts the pictures in its quoted original came from: the original's, for a reply or
    /// forward, or the reopened message's own. The quote shows them as `data:` URIs, and every
    /// save and the send turn those back into the parts they came from.
    pub(super) pictures: Arc<[InlinePart]>,
    /// What its composer is doing, for the feature-adoption count a send makes
    /// (`docs/analytics.md`).
    pub(super) kind: ComposeKind,
    /// The importance every save and the send carry (`importance`).
    pub(super) importance: Importance,
    /// The importance it opened with: a stored draft's own, else normal. What
    /// [`ComposerImportance::changed`](crate::ComposerImportance::changed) compares against.
    pub(super) opened_importance: Importance,
    /// The message a reply opened on answers, when the composer was opened on it.
    pub(super) answering: Option<MessageRef>,
    /// Its drafted reply (`docs/ai.md` rules 17 to 25).
    pub(super) reply_draft: ReplyDraftHeld,
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
            kind: ComposeKind::New,
            importance: Importance::Normal,
            opened_importance: Importance::Normal,
            answering: None,
            reply_draft: ReplyDraftHeld::default(),
        }
    }
}

/// What a drafted reply reads of its composition, and its held state to change.
pub(crate) struct ReplyOf<'a> {
    /// The account the composition opened on.
    pub(crate) account: &'a AccountId,
    pub(crate) kind: ComposeKind,
    /// The message answered, when the composer was opened on it.
    pub(crate) answering: Option<&'a MessageRef>,
    /// The `Message-ID` it answers, from its headers: a reply resumed from Drafts names its
    /// original only by this.
    pub(crate) in_reply_to: Option<&'a MessageIdHeader>,
    pub(crate) held: &'a mut ReplyDraftHeld,
}

impl<P: Provider> App<P> {
    /// `change` over what a drafted reply holds of `composition`, or `None` for a composition the
    /// core was never told of or has forgotten.
    pub(crate) fn reply_of<T>(
        &self,
        composition: &CompositionId,
        change: impl FnOnce(ReplyOf<'_>) -> T,
    ) -> Option<T> {
        let mut state = self.drafts.lock().expect("drafts mutex poisoned");
        let open = state.open.get_mut(composition)?;
        Some(change(ReplyOf {
            account: &open.account,
            kind: open.kind,
            answering: open.answering.as_ref(),
            in_reply_to: open
                .threading
                .as_ref()
                .and_then(|threading| threading.in_reply_to.as_ref()),
            held: &mut open.reply_draft,
        }))
    }
}

/// The headers that place a message in its conversation. A reply names the message it answers; a
/// forward carries only the chain, because it continues the conversation without answering anyone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Threading {
    pub(crate) in_reply_to: Option<MessageIdHeader>,
    pub(crate) references: Vec<MessageIdHeader>,
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
    /// Compositions that were discarded or sent. A save naming one stores nothing: its composer
    /// can outlive the discard by a moment, and a save then would put back the draft the user
    /// threw away, under a key nothing holds.
    pub(super) finished: HashSet<CompositionId>,
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
