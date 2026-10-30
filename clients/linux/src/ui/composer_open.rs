//! Opening a composer, every way there is: the core is asked once
//! ([`mailcal_bindings::MailcalApp::open_composer`]), and its answer becomes the composer's context
//! through one builder ([`ComposeContext::from_opening`]).
//!
//! A new message, a reply, a reply-all, a forward, a Drafts row, a mail link, a share and an
//! assistant's draft all come through [`AppModel::open_composer_on`]; an Outbox edit arrives as the
//! same answer already made and joins at [`AppModel::place_composer`].

use std::{os::unix::fs::PermissionsExt, path::PathBuf};

use mailcal_bindings::{ComposeOpening, ComposeOrigin, ComposePrefill, MailcalApp};

use super::{
    AppInput, AppModel,
    composer_draft::PendingNavigation,
    composer_model::{ComposeContext, ComposeKind, PickedFile, new_composition},
    model::OpenedMessage,
    reader::{ComposerHost, ReadingSource},
};
use crate::l10n;

/// Where a composer being opened will be drawn. A window's id is taken only once the core has
/// answered, so a composer that never opens spends none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComposerTarget {
    Pane,
    Window,
}

/// What the core answered a composer's opening with, back on the GTK thread.
pub(crate) struct ComposerOpened {
    pub(crate) target: ComposerTarget,
    /// Whether it was a stored draft being reopened, which has its own failure line.
    pub(crate) draft: bool,
    pub(crate) opening: Result<ComposeOpening, ()>,
    /// Files the composer opens holding on top of the ones the core staged: a share's.
    pub(crate) extra_files: Vec<PickedFile>,
}

impl AppModel {
    /// Opens a composer for what `source` has open: an answer to it, or a new message.
    ///
    /// A draft raised in the mailbox window replaces the reading pane, which is the shipped
    /// behaviour (`docs/capabilities.md`, "Composing keeps the mailbox live"); one raised inside a
    /// reading window opens in a window of its own, because the pane's composer would put the
    /// draft behind whatever the user is looking at (`docs/reading-window.md`). A new message
    /// always opens in the pane: only an answer to a window gets a window.
    pub(super) fn begin_compose(
        &self,
        source: &ReadingSource,
        kind: ComposeKind,
        sender: &relm4::Sender<AppInput>,
    ) {
        let Some(origin) = compose_origin(
            kind,
            self.reader_message(source),
            self.snapshot.selected_account.as_deref(),
        ) else {
            return;
        };
        let target = match source {
            ReadingSource::Pane => ComposerTarget::Pane,
            ReadingSource::Window(_) => ComposerTarget::Window,
        };
        let prefill = showcase_prefill(&origin);
        self.open_composer_on(origin, prefill, target, Vec::new(), sender);
    }

    /// Asks the core to open a composer on `origin`, and opens it on the answer
    /// ([`AppInput::ComposerOpened`]).
    ///
    /// Off the GTK thread: a reply may fetch the message it quotes, a forward and a draft stage
    /// their files, and a large one would freeze the window. The composer appears when the
    /// answer does, holding everything, because one shown before its files arrived could be sent,
    /// or a draft saved, without them.
    pub(super) fn open_composer_on(
        &self,
        origin: ComposeOrigin,
        prefill: Option<ComposePrefill>,
        target: ComposerTarget,
        extra_files: Vec<PickedFile>,
        sender: &relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let sender = sender.clone();
        let draft = matches!(origin, ComposeOrigin::Draft { .. });
        std::thread::spawn(move || {
            let composition = new_composition();
            let directory = staging_dir(&composition);
            // Owner-only, before the core writes any of the user's mail into it, exactly as for
            // an attachment decoded to be opened.
            if std::fs::create_dir_all(&directory).is_ok() {
                let _ =
                    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700));
            }
            let opening = app
                .open_composer(
                    composition,
                    origin,
                    directory.to_string_lossy().into_owned(),
                    prefill,
                )
                .map_err(|_| ());
            // Removes the directory only when the core staged nothing into it.
            let _ = std::fs::remove_dir(&directory);
            sender.emit(AppInput::ComposerOpened(Box::new(ComposerOpened {
                target,
                draft,
                opening,
                extra_files,
            })));
        });
    }

    /// Opens the composer the core answered with, or says that it could not.
    ///
    /// Nothing opens on a failure: an empty composer on a stored draft would replace it on its
    /// next save.
    pub(super) fn composer_opened(&mut self, opened: ComposerOpened) {
        let ComposerOpened {
            target,
            draft,
            opening,
            extra_files,
        } = opened;
        let (Some(app), Ok(opening)) = (self.app.clone(), opening) else {
            self.notice = Some(
                if draft {
                    l10n::compose_draft_open_failed()
                } else {
                    l10n::compose_open_failed()
                }
                .to_owned(),
            );
            return;
        };
        let host = match target {
            ComposerTarget::Pane => ComposerHost::Pane,
            ComposerTarget::Window => ComposerHost::Window(self.next_composer_window()),
        };
        let mut request = self.compose_context(&app, opening, host);
        request.files.extend(extra_files);
        self.place_composer(request);
    }

    /// The composer's context for what the core opened it with.
    pub(super) fn compose_context(
        &self,
        app: &MailcalApp,
        opening: ComposeOpening,
        host: ComposerHost,
    ) -> ComposeContext {
        ComposeContext::from_opening(
            opening,
            host,
            &app.quote_settings().style,
            self.calendar.display_zone(),
        )
    }

    /// Puts an opened composer on screen where it belongs. A composer already in the pane is
    /// left first, so what it holds is kept in Drafts.
    pub(super) fn place_composer(&mut self, request: ComposeContext) {
        match request.host {
            ComposerHost::Pane if self.composer.is_some() => {
                self.queue_navigation(PendingNavigation::Composer(request));
            }
            ComposerHost::Pane => self.commit_composer(request),
            ComposerHost::Window(id) => self.open_composer_window(id, request),
        }
    }
}

/// What a composer of `kind` raised over `opened` is opened on: the open message for a reply or a
/// forward, and a new message from the folder's account otherwise, whatever the reading pane
/// shows. `None` for an answer with nothing open to answer.
fn compose_origin(
    kind: ComposeKind,
    opened: Option<&OpenedMessage>,
    selected_account: Option<&str>,
) -> Option<ComposeOrigin> {
    if kind == ComposeKind::New {
        return Some(ComposeOrigin::New {
            account: selected_account.map(str::to_owned),
        });
    }
    let message = opened?;
    Some(answer_origin(
        kind,
        message.account.clone(),
        message.key.clone(),
    ))
}

/// What an answer of `kind` to one message is opened on: a forward, or a reply to its sender or
/// to everyone.
pub(super) fn answer_origin(kind: ComposeKind, account: String, key: String) -> ComposeOrigin {
    match kind {
        ComposeKind::Forward => ComposeOrigin::Forward { account, key },
        _ => ComposeOrigin::Reply {
            account,
            key,
            all: kind == ComposeKind::ReplyAll,
        },
    }
}

/// The sample answer a showcase run puts above the quote of its designated reply, so the store
/// screenshot shows a written reply rather than an empty body. `None` for every real reply.
#[cfg(any(debug_assertions, feature = "dev-harness"))]
fn showcase_prefill(origin: &ComposeOrigin) -> Option<ComposePrefill> {
    let ComposeOrigin::Reply { account, key, .. } = origin else {
        return None;
    };
    crate::showcase::reply_text(account, key).map(|body_text| ComposePrefill {
        body_text,
        ..ComposePrefill::default()
    })
}

#[cfg(not(any(debug_assertions, feature = "dev-harness")))]
fn showcase_prefill(_origin: &ComposeOrigin) -> Option<ComposePrefill> {
    None
}

/// A directory of this composer's own under the user's cache, so two composers never share a
/// staged file. Under the **cache** directory rather than `/tmp`, which inside a Flatpak is the
/// sandbox's own, the same rule an opened attachment follows ([`super::operations`]).
fn staging_dir(composition: &str) -> PathBuf {
    gtk::glib::user_cache_dir()
        .join("mailcal")
        .join(format!("composer-{}-{composition}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::ComposeOrigin;

    use super::{ComposeKind, compose_origin};
    use crate::ui::model::OpenedMessage;

    fn open_message() -> OpenedMessage {
        OpenedMessage {
            account: "message-account".to_owned(),
            key: "message".to_owned(),
            subject: "Planning".to_owned(),
            from: "Sender <sender@example.test>".to_owned(),
            date: "2026-07-20".to_owned(),
            avatar: crate::ui::avatar::AvatarData::from(&crate::ui::model::blank_avatar()),
        }
    }

    /// New Mail with a message open in the reading pane: nothing to quote or answer, and the
    /// account of the folder on screen rather than of the message beside it.
    #[test]
    fn a_new_message_answers_nothing_even_with_a_message_open() {
        let message = open_message();
        assert_eq!(
            compose_origin(ComposeKind::New, Some(&message), Some("selected-account")),
            Some(ComposeOrigin::New {
                account: Some("selected-account".to_owned())
            })
        );
        assert_eq!(
            compose_origin(ComposeKind::New, None, None),
            Some(ComposeOrigin::New { account: None }),
            "where the list belongs to no account, the core decides"
        );
    }

    #[test]
    fn a_reply_or_forward_answers_the_open_message() {
        let message = open_message();
        assert_eq!(
            compose_origin(ComposeKind::Reply, Some(&message), Some("selected")),
            Some(ComposeOrigin::Reply {
                account: "message-account".to_owned(),
                key: "message".to_owned(),
                all: false,
            })
        );
        assert_eq!(
            compose_origin(ComposeKind::ReplyAll, Some(&message), None),
            Some(ComposeOrigin::Reply {
                account: "message-account".to_owned(),
                key: "message".to_owned(),
                all: true,
            })
        );
        assert_eq!(
            compose_origin(ComposeKind::Forward, Some(&message), None),
            Some(ComposeOrigin::Forward {
                account: "message-account".to_owned(),
                key: "message".to_owned(),
            })
        );
        for kind in [
            ComposeKind::Reply,
            ComposeKind::ReplyAll,
            ComposeKind::Forward,
        ] {
            assert_eq!(
                compose_origin(kind, None, Some("selected")),
                None,
                "{kind:?}"
            );
        }
    }

    /// The twin of `operations`' assertion for an opened attachment, and it fails the same way:
    /// a path under the sandbox's private `/tmp` looks right in a host build and is not there in
    /// the Flatpak the user runs, so a draft opens without its files and the next save takes
    /// them off the server.
    #[test]
    fn a_staged_file_never_lands_in_the_sandboxs_private_tmp() {
        let directory = super::staging_dir("composer-1");
        assert!(
            directory.starts_with(gtk::glib::user_cache_dir()),
            "a staged file belongs under the app's cache directory: {directory:?}"
        );
        assert!(
            !directory.starts_with(std::env::temp_dir()),
            "and never under /tmp, which a sandbox does not share: {directory:?}"
        );
    }

    /// Two composers never share a directory, so removing one composer's staged file cannot take
    /// another's.
    #[test]
    fn each_composer_stages_into_a_directory_of_its_own() {
        assert_ne!(
            super::staging_dir("composer-1"),
            super::staging_dir("composer-2")
        );
    }
}
