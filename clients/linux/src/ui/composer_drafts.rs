//! Keeping the composer's message on the server: the core verbs, the idle timer that stores the
//! draft, and the hint the composer draws (`docs/drafts.md`).
//!
//! Every one of them names a composition, which is this host's handle on one open composer. The
//! core mints none: a composer takes an id when it opens and keeps it until it closes, and that
//! is what lets two composers save without superseding each other's draft.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    os::unix::fs::PermissionsExt as _,
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

use gtk::gio;
use mailcal_bindings::{DraftStatus, draft_autosave_idle_seconds};
use webkit6::prelude::WebViewExt;

use super::{
    AppModel,
    composer_fields::ComposerFields,
    composer_model::{
        ComposeContext, ComposeKind, ComposerSubmission, PickedFile, new_composition,
    },
    model,
    reader::ComposerHost,
};
use crate::l10n;

/// How many samples one idle interval is counted in.
///
/// Three, so the lag a sampler costs is a third of the interval: a draft reaches the server
/// between one and one-and-a-third intervals after the last keystroke, and never during typing.
const SAMPLES_PER_INTERVAL: u32 = 3;

impl AppModel {
    /// Stores what the composer holds in the Drafts folder, replacing what this composition's
    /// previous save left there.
    ///
    /// Always the file-carrying call, even from a composer holding none. A save replaces the
    /// stored copy, so one that left the files out would take them off a draft the user is still
    /// writing, and a composer can pick a file at any moment.
    ///
    /// Fire and forget: it returns as soon as the document validates, and the outcome arrives as
    /// a `Surface::DraftStatus` signal. A save with no network is queued, not lost.
    pub(super) fn save_composer_draft(&mut self, submission: &ComposerSubmission) {
        let Some(app) = &self.app else {
            return;
        };
        let recipients = mailcal_bindings::Recipients {
            to: submission.to.clone(),
            cc: submission.cc.clone(),
            bcc: submission.bcc.clone(),
        };
        let files = submission
            .files
            .iter()
            .map(|file| mailcal_bindings::ComposerFileAttachment {
                path: file.path.clone(),
                file_name: file.file_name.clone(),
                media_type: file.media_type.clone(),
            })
            .collect();
        // Nothing is surfaced on a refusal. Saving is never something the user waits for, and
        // never something a composer refuses to be dismissed over; a save the *server* refuses is
        // reported through the hint.
        let _ = app.save_draft_with_files(
            submission.request.composition.clone(),
            recipients,
            submission.subject.clone(),
            submission.document_json.clone(),
            files,
            submission.from.clone(),
        );
    }

    /// Removes a composition's stored draft from the server and forgets the composition.
    ///
    /// The one path that takes the stored copy off the server; closing the composer any other way
    /// leaves the draft in Drafts, which is what a resumed draft the user only looked at needs.
    pub(super) fn discard_stored_draft(&self, composition: &str) {
        if let Some(app) = &self.app {
            let _ = app.discard_draft(composition.to_owned());
        }
    }

    /// Forgets a composition, leaving the stored draft where it is.
    ///
    /// Called however the composer went, sent, discarded or dismissed. Without it the core holds
    /// a record per composer for the life of the process, and the stored draft would be
    /// superseded by whatever the next composer to take this id wrote.
    pub(super) fn close_composition(&mut self, composition: &str) {
        self.draft_status.remove(composition);
        if let Some(app) = &self.app {
            let _ = app.close_composition(composition.to_owned());
        }
    }

    /// Takes the pane's composer off screen, forgetting its composition.
    ///
    /// Every way a composer closes **without sending** comes through here: one that went without
    /// it leaves the core holding a record for the life of the process, and the stored draft
    /// would be superseded by whatever the next composer to take that id wrote. The draft itself
    /// stays in Drafts; only Discard removes it. A composer whose message was submitted is taken
    /// off screen by the submit instead, which leaves the composition to the send.
    pub(super) fn clear_pane_composer(&mut self) {
        if let Some(request) = self.composer.take() {
            self.close_composition(&request.composition);
        }
        self.composer_error = None;
    }

    /// Re-pulls every open composer's own state.
    ///
    /// A `Surface::DraftStatus` signal says that *some* composition's save moved, not which, so
    /// each composer is asked about its own: one that has saved nothing reads `Idle`, never the
    /// composer beside it. The same rule the reading windows follow.
    pub(super) fn pull_draft_status(&mut self) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let compositions: Vec<String> = self
            .composer
            .iter()
            .chain(self.composer_windows.iter().map(|draft| &draft.request))
            .map(|request| request.composition.clone())
            .collect();
        for composition in compositions {
            let status = app
                .draft_status(composition.clone())
                .unwrap_or(DraftStatus::Idle);
            self.draft_status.insert(composition, status);
        }
        self.draft_status_generation = self.draft_status_generation.wrapping_add(1);
    }

    /// Opens a Drafts-folder row back into its composer, and every other row for reading.
    ///
    /// Told by the **folder**, not by the row: the core answers per message but the list does not
    /// carry it, so a draft met in a search result or in another folder's thread opens read-only
    /// (`docs/drafts.md`, known gaps).
    ///
    /// Resuming is a round trip and, unlike a forward, cannot count on the message being cached:
    /// a draft is opened from a list row, so the first open of one fetches it. It runs on a
    /// thread of its own for that reason, and the composer appears when the answer does.
    pub(super) fn open_or_resume(
        &mut self,
        message: model::OpenedMessage,
        sender: relm4::Sender<super::AppInput>,
    ) {
        if !self.snapshot.showing_drafts {
            self.open_message(message);
            return;
        }
        let Some(app) = self.app.clone() else {
            return;
        };
        let composition = new_composition();
        let directory = resume_staging_dir(&composition);
        let account = message.account.clone();
        let key = message.key.clone();
        let id = composition.clone();
        std::thread::spawn(move || {
            // Owner-only, before the core writes any of the user's mail into it, exactly as for
            // a forward's files and for an attachment decoded to be opened.
            let prepared = std::fs::create_dir_all(&directory).map(|()| {
                let _ =
                    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700));
            });
            let resumed = prepared.map_err(|_| ()).and_then(|()| {
                app.resume_draft(
                    id.clone(),
                    account,
                    key,
                    directory.to_string_lossy().into_owned(),
                )
                .map_err(|_| ())
            });
            sender.emit(super::AppInput::DraftResumed(id, Box::new(resumed)));
        });
    }

    /// Shows a resumed draft, or says that it could not be opened.
    ///
    /// Nothing is adopted on a failure, so the message is left alone rather than opened into a
    /// composer whose next save would replace it with whatever was on screen.
    pub(super) fn draft_resumed(
        &mut self,
        composition: String,
        resumed: Result<mailcal_bindings::DraftResume, ()>,
    ) {
        match resumed {
            Ok(draft) => self.commit_composer(resumed_context(composition, draft)),
            Err(()) => self.notice = Some(l10n::compose_draft_open_failed().to_owned()),
        }
    }

    /// How the draft in the composer hosted by `host` was last saved, or `None` when that
    /// composer has saved nothing, or is not open.
    pub(super) fn draft_status_of(&self, host: ComposerHost) -> Option<DraftStatus> {
        let request = match host {
            ComposerHost::Pane => self.composer.as_ref()?,
            ComposerHost::Window(id) => {
                &self
                    .composer_windows
                    .iter()
                    .find(|draft| draft.id == id)?
                    .request
            }
        };
        self.draft_status.get(&request.composition).copied()
    }
}

/// A directory of this composer's own under the user's cache, so two resumed drafts never share
/// a staged file.
///
/// Under the **cache** directory rather than `/tmp`, which inside a Flatpak is the sandbox's own:
/// the same rule a forward's staging and an opened attachment follow.
fn resume_staging_dir(composition: &str) -> PathBuf {
    gtk::glib::user_cache_dir().join("mailcal").join(format!(
        "resumed-draft-{}-{composition}",
        std::process::id()
    ))
}

/// What a resumed draft's composer opens with.
///
/// Two things separate it from every other new message. The composition is the one the core
/// adopted the stored draft into, never a fresh one, or the composer's first save would store a
/// second copy beside the one it is showing. And it seeds **no signature**, for the reason a
/// withdrawn message does not: the body came back as the text of a message that was signed when
/// it was first written, so seeding one would put a second signature under it, and the next save
/// would store that.
fn resumed_context(composition: String, draft: mailcal_bindings::DraftResume) -> ComposeContext {
    ComposeContext {
        kind: ComposeKind::New,
        host: ComposerHost::Pane,
        account: None,
        key: None,
        initial_to: draft.to,
        initial_cc: draft.cc,
        initial_bcc: draft.bcc,
        subject: draft.subject,
        initial_body: (!draft.body_text.is_empty()).then_some(draft.body_text),
        quote: None,
        initial_from: Some(draft.account).filter(|account| !account.is_empty()),
        seeds_signature: false,
        files: draft
            .attachments
            .into_iter()
            .map(|file| PickedFile {
                path: file.path,
                file_name: file.file_name,
                media_type: file.media_type,
            })
            .collect(),
        composition,
    }
}

/// The quiet line under the editor, or `None` when there is nothing to say.
///
/// A hint and never a gate: no state here stops the composer being closed, and none of it is
/// worth a dialog. A composer that has saved nothing says nothing.
pub(crate) fn draft_hint_text(status: Option<DraftStatus>) -> Option<&'static str> {
    match status? {
        DraftStatus::Idle => None,
        DraftStatus::Saving => Some(l10n::compose_draft_saving()),
        DraftStatus::Saved => Some(l10n::compose_draft_saved()),
        DraftStatus::Queued => Some(l10n::compose_draft_queued()),
        DraftStatus::Failed => Some(l10n::compose_draft_failed()),
    }
}

/// Whether the hint is reporting something that did not work, the one state drawn in the error
/// colour.
pub(crate) fn draft_hint_failed(status: Option<DraftStatus>) -> bool {
    matches!(status, Some(DraftStatus::Failed))
}

/// One composer's autosave: the single timer that notices a change and stores the message once
/// the composer has gone quiet.
///
/// One timer rather than two. The editor is sampled on every tick (one integer across the bridge)
/// and the interval is counted in ticks, so there is a single source to remove on teardown and no
/// second one that could outlive the editor it reads.
#[derive(Default)]
pub(crate) struct Autosave {
    source: RefCell<Option<gtk::glib::SourceId>>,
}

impl Autosave {
    /// Starts the timer for one composer. `store` runs once the composer has been quiet for the
    /// core's interval, and nothing runs at all until something has changed: a composer nobody
    /// has touched never saves.
    pub(crate) fn start(
        &self,
        editor: &webkit6::WebView,
        fields: ComposerFields,
        store: impl Fn() + 'static,
    ) {
        self.stop();
        let interval = draft_autosave_idle_seconds().max(1);
        let sample = Duration::from_secs(interval).div_f64(f64::from(SAMPLES_PER_INTERVAL));
        let editor = editor.clone();
        // The editor's answer arrives on a later turn of the loop, so it lands in a flag the next
        // tick reads rather than being returned to the tick that asked.
        let revision_moved = Rc::new(Cell::new(false));
        let seen_revision = Rc::new(Cell::new(-1));
        let mut seen_header = fields.fingerprint();
        let mut quiet = 0_u32;
        let mut dirty = false;
        let source = gtk::glib::timeout_add_local(sample, move || {
            sample_editor(&editor, &seen_revision, &revision_moved);
            let header = fields.fingerprint();
            let moved = revision_moved.replace(false) || header != seen_header;
            seen_header = header;
            if moved {
                quiet = 0;
                dirty = true;
                return gtk::glib::ControlFlow::Continue;
            }
            if dirty {
                quiet += 1;
                if quiet >= SAMPLES_PER_INTERVAL {
                    quiet = 0;
                    dirty = false;
                    store();
                }
            }
            gtk::glib::ControlFlow::Continue
        });
        self.source.replace(Some(source));
    }

    /// Stops the timer, if one is running. Called as each composer is torn down, so no tick
    /// outlives the editor it reads.
    pub(crate) fn stop(&self) {
        if let Some(source) = self.source.take() {
            source.remove();
        }
    }
}

/// Reads the editor's change count and reports a move.
///
/// The message body is a WebView with no channel back to this host, so the shared editor bundle
/// counts its own mutations and this reads the count.
fn sample_editor(editor: &webkit6::WebView, seen: &Rc<Cell<i32>>, moved: &Rc<Cell<bool>>) {
    let seen = Rc::clone(seen);
    let moved = Rc::clone(moved);
    editor.evaluate_javascript(
        "window.composerRevision()",
        None,
        None,
        None::<&gio::Cancellable>,
        move |result| {
            let Ok(value) = result else {
                return;
            };
            let Ok(revision) = value.to_str().trim().parse::<i32>() else {
                return;
            };
            let previous = seen.replace(revision);
            // The first answer is the baseline the editor loaded with, never an edit: without it
            // every composer would store a draft moments after opening.
            if previous >= 0 && previous != revision {
                moved.set(true);
            }
        },
    );
}

/// Every composition a host is holding a status for, so the map does not grow with the session.
pub(crate) type DraftStatuses = HashMap<String, DraftStatus>;

#[cfg(test)]
mod tests {
    /// The twin of the forward staging assertion, and it fails the same way: a path under the
    /// sandbox's private `/tmp` looks right in a host build and is not there in the Flatpak the
    /// user runs, so the composer opens without the files the draft carries and the next save
    /// takes them off the server.
    #[test]
    fn a_staged_draft_file_never_lands_in_the_sandboxs_private_tmp() {
        let directory = super::resume_staging_dir("composer-1");
        assert!(
            directory.starts_with(gtk::glib::user_cache_dir()),
            "staged under {}, which a Flatpak does not share with the host",
            directory.display()
        );
    }

    /// Two resumed drafts never share a directory, so removing one composer's staged file cannot
    /// take another composer's.
    #[test]
    fn each_resumed_draft_stages_into_a_directory_of_its_own() {
        assert_ne!(
            super::resume_staging_dir("composer-1"),
            super::resume_staging_dir("composer-2")
        );
    }
}
