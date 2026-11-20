//! The drafted replies this device knows of (`docs/ai.md` rule 25), held in memory and written
//! behind each one, so a send or a save never waits on the disk for it; and the mark a drafted
//! composition's stored draft carries.

use std::{collections::BTreeSet, io, path::Path};

use engine_api::{AccountId, Keyword, MessageIdHeader, Provider};
use mailcal_account::{
    DraftedReplies, drafted_replies_path, drafted_reply_keyword, load_drafted_replies,
    save_drafted_replies,
};

use crate::{
    App, CompositionId,
    preferences_file::saved_file::{Document, SavedFile},
};

/// A handle on the log's file.
pub(crate) type DraftedFile = SavedFile<DraftedReplies>;

impl Document for DraftedReplies {
    const NAME: &'static str = "drafted replies";

    fn read(path: &Path) -> Self {
        load_drafted_replies(path)
    }

    fn write(path: &Path, document: &Self) -> io::Result<()> {
        save_drafted_replies(path, document)
    }
}

/// The log as the app holds it, and its file.
pub(crate) struct DraftedLog {
    log: DraftedReplies,
    file: DraftedFile,
}

impl DraftedLog {
    pub(super) fn new(dir: Option<&Path>, on_failure: impl Fn() + Send + Sync + 'static) -> Self {
        let path = dir.map(drafted_replies_path);
        Self {
            log: path.as_ref().map(load_drafted_replies).unwrap_or_default(),
            file: DraftedFile::at(path, on_failure),
        }
    }
}

impl<P: Provider> App<P> {
    fn drafted_log(&self) -> std::sync::MutexGuard<'_, DraftedLog> {
        self.ai
            .drafted
            .lock()
            .expect("drafted replies mutex poisoned")
    }

    /// Notes the reply `message_id`, sent or stored from `account`, as drafted. Written behind the
    /// caller, and only when the log does not name it yet.
    pub fn note_drafted_reply(&self, account: &str, message_id: MessageIdHeader) {
        let mut drafted = self.drafted_log();
        if drafted.log.is_drafted(&message_id) {
            return;
        }
        drafted.log.note(message_id.clone(), account);
        let account = account.to_owned();
        drafted
            .file
            .edit(move |log| log.note(message_id.clone(), &account));
    }

    /// Whether the message `message_id` was sent from this device from a draft. A message is
    /// drafted when this says so **or** it carries the drafted-reply keyword.
    #[must_use]
    pub fn is_drafted_reply(&self, message_id: &MessageIdHeader) -> bool {
        self.drafted_log().log.is_drafted(message_id)
    }

    /// Drops the drafted replies `account` sent; called as the account is removed.
    pub(crate) fn forget_drafted_replies(&self, account: &str) {
        let mut drafted = self.drafted_log();
        if drafted.log.forget_account(account) {
            let account = account.to_owned();
            drafted.file.edit(move |log| {
                log.forget_account(&account);
            });
        }
    }

    pub(crate) fn drafted_replies_file(&self) -> DraftedFile {
        self.drafted_log().file.clone()
    }

    /// The keywords `composition`'s stored draft carries: the drafted-reply keyword while the
    /// composition is drafted, otherwise none. The whole set goes with every save, since a keyword
    /// a save leaves out is taken off the stored copy.
    pub(crate) fn draft_keywords(&self, composition: &CompositionId) -> BTreeSet<Keyword> {
        if self.composition_drafted(composition) {
            BTreeSet::from([drafted_reply_keyword()])
        } else {
            BTreeSet::new()
        }
    }

    /// After a save of `message_id` from `account` that asked for `asked`: a drafted-reply keyword
    /// the answer does not say was `kept` is noted in this device's log, so this device still knows
    /// the draft for what it is after a restart. `kept` is `None` for a save queued for later,
    /// whose keywords only a sync reads back.
    pub(crate) fn drafted_save_settled(
        &self,
        account: &AccountId,
        message_id: &MessageIdHeader,
        asked: &BTreeSet<Keyword>,
        kept: Option<&BTreeSet<Keyword>>,
    ) {
        let keyword = drafted_reply_keyword();
        if asked.contains(&keyword) && !kept.is_some_and(|kept| kept.contains(&keyword)) {
            log::info!(
                "ai: the server kept no mark on a drafted reply's draft; noted on this device"
            );
            self.note_drafted_reply(account.as_str(), message_id.clone());
        }
    }
}
