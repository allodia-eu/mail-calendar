//! One pass over the difference between this device's writing styles and the person's other
//! devices', run after the account pass from the same call (`docs/ai.md`).
//!
//! **Quieter than the account pass.** A style needs no password, so what arrives is applied here
//! without asking: a new style is created, a changed one updated, a forgotten one forgotten. The
//! one thing that reaches a person is a style changed on both sides, reported as an
//! [`AllodiaStyleConflict`] and settled by
//! [`resolve_writing_style_conflict`](MailcalApp::resolve_writing_style_conflict).
//!
//! **Only the name and the guide travel.** [`SyncedStyle`] has no field for a passage, and a style
//! that arrives is given passages from this device's own sent mail.
//!
//! **A failure applies to one style, never to the pass, and never to the account pass before it.**

use allodia_license::{
    AccountService, ConflictWith, Error, Feature, LocalStyle, Refusal, StyleList, StyleRecord,
    SyncState, SyncedCollection, SyncedStyle, Verdict, fingerprint, reconcile_styles,
};

use crate::{
    MailcalApp, MailcalError, allodia_pass::Pass, allodia_sync::AllodiaStyleConflict,
    allodia_transport::HttpsTransport, blocking::block_on, sync_state::StoredSyncState,
};

/// One style pass: the app it applies to, the calls it makes, and whether it may write.
struct StylePass<'a> {
    app: &'a MailcalApp,
    pass: &'a Pass<'a>,
    styles: SyncedCollection<'a, StyleRecord>,
    writes: bool,
}

impl MailcalApp {
    /// Brings this device's writing styles and the service's into step, and returns the styles
    /// changed on both sides. Nothing at all when the sign-in does not include reading them.
    pub(crate) fn sync_writing_styles(&self, pass: &Pass<'_>) -> Vec<AllodiaStyleConflict> {
        if !self.allodia_grant_permits(Feature::ReadWritingStyles) {
            return Vec::new();
        }
        let run = StylePass::new(self, pass);
        let mut remote = match run.styles.list(pass.transport, pass.token, None) {
            Ok(remote) => remote,
            Err(error) => {
                log::warn!("allodia: the writing styles could not be read; {error}");
                return Vec::new();
            }
        };
        let local = run.local();
        run.settle_removals(&local, &mut remote);
        log::info!(
            "allodia: syncing {} writing style(s) against {} held by the service",
            local.len(),
            remote.styles.len(),
        );
        run.apply(&local, &remote)
    }

    /// Settle a style [`MailcalApp::sync_writing_styles`] reported as changed on both sides, from
    /// what the service holds now rather than what it held then.
    pub(crate) fn resolve_style_conflict(
        &self,
        pass: &Pass<'_>,
        style_id: &str,
        keep_this_device: bool,
    ) -> Result<(), MailcalError> {
        let needed = if keep_this_device {
            Feature::WriteWritingStyles
        } else {
            Feature::ReadWritingStyles
        };
        if !self.allodia_grant_permits(needed) {
            return Err(MailcalError::Connect(
                "this device's Allodia sign-in does not include syncing writing styles".to_owned(),
            ));
        }
        let entry = pass.bookkeeping.style(style_id).ok_or_else(|| {
            MailcalError::Config("this writing style has not been synced".to_owned())
        })?;
        let run = StylePass::new(self, pass);
        let remote = run
            .styles
            .list(pass.transport, pass.token, None)
            .map_err(|err| self.report_allodia(err))?;
        let current = remote.styles.iter().find(|record| record.id == entry.id);
        let version = current.map(|record| record.version).or_else(|| {
            remote
                .deleted
                .iter()
                .find(|gone| gone.id == entry.id)
                .map(|gone| gone.version)
        });
        let Some(version) = version else {
            // The service holds nothing under this id, so the next pass sends the style as new.
            run.forget_entry(style_id);
            return Ok(());
        };
        if keep_this_device {
            let style = run
                .local()
                .into_iter()
                .find(|style| style.style_id == style_id)
                .ok_or_else(|| MailcalError::Config("no such writing style".to_owned()))?
                .style;
            // Written over a forgotten record too, which the service takes as bringing it back.
            let stored = run
                .styles
                .update(pass.transport, pass.token, &entry.id, version, &style)
                .map_err(|err| self.report_allodia(err))?;
            run.remember(style_id, &stored, fingerprint(&style));
        } else if let Some(record) = current {
            run.take(style_id, record);
        } else {
            run.forget_here(style_id);
        }
        log::info!(
            "allodia: [style {style_id}] settled, keeping {}",
            if keep_this_device {
                "this device's version"
            } else {
                "the other devices' version"
            }
        );
        Ok(())
    }

    /// [`MailcalApp::resolve_writing_style_conflict`], from the token to the answer.
    pub(crate) fn run_style_conflict_resolution(
        &self,
        style_id: &str,
        keep_this_device: bool,
    ) -> Result<(), MailcalError> {
        let token = self.allodia_access_token("a writing style")?;
        let bookkeeping = self.allodia_bookkeeping()?;
        let transport =
            HttpsTransport::new(self.runtime.handle().clone()).map_err(MailcalError::Connect)?;
        let service = AccountService::new(allodia_license::host());
        let pass = Pass {
            service: &service,
            transport: &transport,
            token: &token,
            bookkeeping: &bookkeeping,
        };
        self.resolve_style_conflict(&pass, style_id, keep_this_device)
    }
}

impl<'a> StylePass<'a> {
    fn new(app: &'a MailcalApp, pass: &'a Pass<'a>) -> Self {
        Self {
            app,
            pass,
            styles: SyncedCollection::new(pass.service),
            writes: app.allodia_grant_permits(Feature::WriteWritingStyles),
        }
    }

    /// This device's styles, as the reconciler sees them.
    fn local(&self) -> Vec<LocalStyle> {
        self.app
            .app
            .syncable_writing_styles()
            .into_iter()
            .map(|style| {
                let sync = self
                    .pass
                    .bookkeeping
                    .style(&style.id)
                    .map(|entry| SyncState {
                        id: entry.id,
                        version: entry.version,
                        fingerprint: entry.fingerprint,
                        detached: false,
                    });
                LocalStyle {
                    style_id: style.id,
                    style: SyncedStyle {
                        name: style.name,
                        guide: style.guide,
                    },
                    sync,
                }
            })
            .collect()
    }

    /// Deal with every style this device has synced and no longer offers.
    ///
    /// One forgotten here is forgotten at the service. Every record that is not settled by that
    /// is kept out of this pass, where it would otherwise arrive as a new style: one the service
    /// could not be told about yet, and one still in the library whose guide does not read.
    fn settle_removals(&self, local: &[LocalStyle], remote: &mut StyleList) {
        for (style_id, entry) in self.pass.bookkeeping.styles() {
            if local.iter().any(|style| style.style_id == style_id) {
                continue;
            }
            let forgotten_here = self.app.app.writing_style_detail(&style_id).is_none();
            let arrives_again =
                forgotten_here && self.writes && self.forget_at_service(&style_id, &entry);
            if !arrives_again {
                remote.styles.retain(|record| record.id != entry.id);
            }
        }
    }

    /// Tell the service a style forgotten here is gone. `true` when another device changed it
    /// since, in which case it is not forgotten there and arrives here again.
    fn forget_at_service(&self, style_id: &str, entry: &StoredSyncState) -> bool {
        match self.styles.delete(
            self.pass.transport,
            self.pass.token,
            &entry.id,
            entry.version,
        ) {
            Ok(()) => {
                self.forget_entry(style_id);
                log::info!("allodia: [style {style_id}] forgotten on the other devices too");
                false
            }
            Err(Error::Conflict(Some(ConflictWith::Style(_)))) => {
                self.forget_entry(style_id);
                log::info!(
                    "allodia: [style {style_id}] was changed on another device after it was \
                     forgotten here, so it comes back"
                );
                true
            }
            Err(error) => {
                log::warn!(
                    "allodia: [style {style_id}] was forgotten here, but the service could not be \
                     told ({error}); it will be told at the next pass"
                );
                false
            }
        }
    }

    /// Do this device's half of each decision, and hand back the styles that need a person.
    fn apply(&self, local: &[LocalStyle], remote: &StyleList) -> Vec<AllodiaStyleConflict> {
        let find = |local_id: &str| local.iter().find(|style| style.style_id == local_id);
        let mut conflicts = Vec::new();
        let mut full = false;
        for verdict in reconcile_styles(local, remote) {
            match verdict {
                Verdict::Upload { local_id } => {
                    if self.writes
                        && !full
                        && let Some(mine) = find(&local_id)
                    {
                        full = !self.upload(&local_id, &mine.style);
                    }
                }
                Verdict::Push {
                    local_id,
                    id,
                    version,
                } => {
                    if self.writes
                        && let Some(mine) = find(&local_id)
                    {
                        self.push(&local_id, &id, version, &mine.style);
                    }
                }
                Verdict::UpdateAvailable { local_id, current } => {
                    self.take(&local_id, &current);
                }
                Verdict::Conflict { local_id, .. } => {
                    if let Some(mine) = find(&local_id) {
                        conflicts.push(AllodiaStyleConflict {
                            name: mine.style.name.clone(),
                            style_id: local_id,
                        });
                    }
                }
                Verdict::Offer { current } => self.receive(&current),
                Verdict::Adopt { local_id, current } => {
                    self.remember(&local_id, &current, fingerprint(&current.style));
                }
                Verdict::RemovedElsewhere { local_id } => self.forget_here(&local_id),
            }
        }
        conflicts
    }

    /// Store a style the service has never seen. `false` when the service holds as many styles as
    /// it takes, so nothing else is worth sending in this pass.
    fn upload(&self, style_id: &str, style: &SyncedStyle) -> bool {
        let Some(key) = self.create_key(style_id) else {
            return true;
        };
        match self
            .styles
            .create(self.pass.transport, self.pass.token, style, &key)
        {
            Ok(record) => {
                self.drop_create_key(style_id);
                self.remember(style_id, &record, fingerprint(style));
                true
            }
            Err(Error::Refused(Refusal::TooManyStyles)) => {
                log::warn!(
                    "allodia: the service holds as many writing styles as it keeps; the rest stay \
                     on this device until one is forgotten"
                );
                false
            }
            Err(error @ Error::Conflict(Some(ConflictWith::Tombstone(_)))) => {
                self.drop_create_key(style_id);
                log::warn!("allodia: [style {style_id}] was forgotten while it was sent; {error}");
                true
            }
            Err(error) => {
                // The key is kept: the answer may be what was lost, and the retry has to present
                // the same one or it stores the style twice.
                log::warn!("allodia: [style {style_id}] could not be sent to the service; {error}");
                true
            }
        }
    }

    /// The key this create presents: the one a previous attempt left, or a fresh one written down
    /// before the request, because the failure it guards against is an answer that never arrives.
    fn create_key(&self, style_id: &str) -> Option<String> {
        if let Some(existing) = self.pass.bookkeeping.pending_style_create_key(style_id) {
            return Some(existing);
        }
        let moment = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
        let key = format!("style-{style_id}-{moment}");
        match self
            .pass
            .bookkeeping
            .set_pending_style_create_key(style_id, Some(&key))
        {
            Ok(()) => Some(key),
            Err(error) => {
                log::error!(
                    "allodia: [style {style_id}] was not sent because its retry key could not be \
                     stored; {error}"
                );
                None
            }
        }
    }

    fn drop_create_key(&self, style_id: &str) {
        if let Err(error) = self
            .pass
            .bookkeeping
            .set_pending_style_create_key(style_id, None)
        {
            log::warn!("allodia: [style {style_id}] the finished create's key stays; {error}");
        }
    }

    /// Replace a record this device has changed, at the version it read.
    fn push(&self, style_id: &str, id: &str, version: u64, style: &SyncedStyle) {
        match self
            .styles
            .update(self.pass.transport, self.pass.token, id, version, style)
        {
            Ok(record) => {
                self.remember(style_id, &record, fingerprint(style));
            }
            // The service has no record under this id; without the entry, the next pass sends the
            // style as new.
            Err(Error::Unexpected { status: 404 }) => {
                self.forget_entry(style_id);
                log::info!(
                    "allodia: [style {style_id}] is unknown to the service; sending it anew"
                );
            }
            Err(error) => {
                log::warn!(
                    "allodia: [style {style_id}] could not be updated at the service; {error}"
                );
            }
        }
    }

    /// Apply what another device changed. `true` when the style is still here to change.
    fn take(&self, style_id: &str, record: &StyleRecord) -> bool {
        let applied = self.app.app.apply_synced_writing_style(
            style_id,
            record.style.name.clone(),
            &record.style.guide,
        );
        if applied {
            self.remember(style_id, record, fingerprint(&record.style));
        }
        applied
    }

    /// Create a style another device synced, with passages from this device's own mail.
    fn receive(&self, record: &StyleRecord) {
        let style_id = block_on(
            self.app.runtime.handle(),
            self.app
                .app
                .add_synced_writing_style(record.style.name.clone(), &record.style.guide),
        );
        self.remember(&style_id, record, fingerprint(&record.style));
    }

    /// Forget a style another device forgot, and every account's choice of it.
    fn forget_here(&self, style_id: &str) {
        self.app.app.delete_writing_style(style_id);
        self.forget_entry(style_id);
        log::info!("allodia: [style {style_id}] was forgotten on another device, and here too");
    }

    /// Write down what this device now knows about a style. `true` when it stuck.
    fn remember(&self, style_id: &str, record: &StyleRecord, base: String) -> bool {
        let state = StoredSyncState {
            id: record.id.clone(),
            version: record.version,
            fingerprint: base,
        };
        match self.pass.bookkeeping.set_style(style_id, state) {
            Ok(()) => true,
            Err(error) => {
                log::error!(
                    "allodia: [style {style_id}] was synced but the note about it could not be \
                     stored ({error})"
                );
                false
            }
        }
    }

    fn forget_entry(&self, style_id: &str) {
        if let Err(error) = self.pass.bookkeeping.forget_style(style_id) {
            log::warn!("allodia: [style {style_id}] the sync note could not be cleared; {error}");
        }
    }
}

#[cfg(test)]
#[path = "allodia_style_pass_tests.rs"]
mod tests;
