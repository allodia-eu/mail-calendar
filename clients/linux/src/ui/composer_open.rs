//! Everything fixed at the moment a composer opens: who it is addressed to, what it quotes, what
//! it is called, and which account it will go out as.
//!
//! Split out of [`super`] so the shell file stays the Relm4 component; this is the one place the
//! four compose entry points (new, reply, reply-all, forward) agree on a request.

use std::{
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    AppInput, AppModel,
    composer_model::{ComposeKind, ComposeRequest, PickedFile, initial_sender, quote_seed},
    composer_notice::ComposerNotice,
};

impl AppModel {
    /// Stages the files the open message carries, then opens its forward composer on the answer
    /// ([`AppInput::ForwardStaged`]).
    ///
    /// Off the GTK thread: the core decodes and writes every file, and a large one would freeze
    /// the window. It reads from the raw source the reading view has already cached, so in the
    /// ordinary case there is nothing to wait for.
    pub(super) fn stage_forward(&self, sender: relm4::Sender<AppInput>) {
        let (Some(app), Some(opened)) = (&self.app, self.reading.opened.as_ref()) else {
            return;
        };
        let app = Arc::clone(app);
        let account = opened.account.clone();
        let key = opened.key.clone();
        std::thread::spawn(move || {
            let directory = forward_staging_dir();
            // Owner-only, before the core writes any mail into it, exactly as for an attachment
            // decoded to be opened.
            if std::fs::create_dir_all(&directory).is_ok() {
                let _ =
                    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700));
            }
            let staged = app
                .stage_forwarded_attachments(account, key, directory.to_string_lossy().into_owned())
                .map(|files| {
                    files
                        .into_iter()
                        .map(|file| PickedFile {
                            path: file.path,
                            file_name: file.file_name,
                            media_type: file.media_type,
                        })
                        .collect()
                })
                .map_err(|_| ());
            sender.emit(AppInput::ForwardStaged(staged));
        });
    }

    /// Opens the forward composer on what staging answered: the files it wrote, or the line that
    /// says they could not be read. An empty list here means the message had nothing attached.
    pub(super) fn begin_forward(&mut self, staged: Result<Vec<PickedFile>, ()>) {
        let failed = staged.is_err();
        self.begin_compose_with(ComposeKind::Forward, staged.unwrap_or_default());
        if failed {
            self.composer_error = Some(ComposerNotice::ForwardAttachments);
        }
    }

    pub(super) fn begin_compose(&mut self, kind: ComposeKind) {
        self.begin_compose_with(kind, Vec::new());
    }

    fn begin_compose_with(&mut self, kind: ComposeKind, files: Vec<PickedFile>) {
        let Some(app) = &self.app else {
            return;
        };
        let opened = self.reading.opened.as_ref();
        if kind != ComposeKind::New && opened.is_none() {
            return;
        }
        let (initial_to, initial_cc) = match (kind, opened) {
            (ComposeKind::Reply | ComposeKind::ReplyAll, Some(message)) => {
                let recipients = app.reply_recipients(
                    message.account.clone(),
                    message.key.clone(),
                    kind == ComposeKind::ReplyAll,
                );
                (recipients.to, recipients.cc)
            }
            _ => (String::new(), String::new()),
        };
        let quote = opened.and_then(|message| {
            let settings = app.quote_settings();
            // Only a showcase reply arrives pre-written, so the store screenshot shows a written
            // reply rather than an empty body. Every real reply passes `None`.
            #[cfg(any(debug_assertions, feature = "dev-harness"))]
            let initial_text = (kind == ComposeKind::Reply || kind == ComposeKind::ReplyAll)
                .then(|| crate::showcase::reply_text(&message.account, &message.key))
                .flatten();
            #[cfg(not(any(debug_assertions, feature = "dev-harness")))]
            let initial_text: Option<String> = None;
            quote_seed(
                message,
                &self.reading.snapshot,
                &settings.style,
                kind == ComposeKind::Forward,
                initial_text.as_deref(),
                self.calendar.display_zone(),
            )
        });
        // Derived by the CORE, not here. The field is editable, so what it opens with is what gets
        // sent unless the user changes it, and a client-side "Re: " + subject differs from the
        // core's on a reply to a reply.
        let subject = match (kind, opened) {
            (ComposeKind::Reply | ComposeKind::ReplyAll, Some(message)) => {
                mailcal_bindings::reply_subject(message.subject.clone())
            }
            (ComposeKind::Forward, Some(message)) => {
                mailcal_bindings::forward_subject(message.subject.clone())
            }
            _ => String::new(),
        };
        self.composer_generation = self.composer_generation.wrapping_add(1);
        self.composer_error = None;
        self.composer = Some(ComposeRequest {
            kind,
            account: opened.map(|message| message.account.clone()),
            key: opened.map(|message| message.key.clone()),
            initial_to,
            initial_cc,
            initial_bcc: String::new(),
            subject,
            initial_body: None,
            quote,
            initial_from: initial_sender(
                opened,
                self.snapshot.selected_account.as_deref(),
                app.default_send_account(),
            ),
            seeds_signature: true,
            // Empty for every route but a share and a forward, which open holding files.
            files,
        });
    }
}

/// A directory of this composer's own under the user's cache, so two forwards never share a
/// staged file. Under the **cache** directory rather than `/tmp`, which inside a Flatpak is the
/// sandbox's own, the same rule an opened attachment follows ([`super::operations`]).
fn forward_staging_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    gtk::glib::user_cache_dir()
        .join("mailcal")
        .join(format!("forward-{}-{nonce}", std::process::id()))
}

#[cfg(test)]
mod tests {
    /// The twin of `operations`' assertion for an opened attachment, and it fails the same way:
    /// a path under the sandbox's private `/tmp` looks right in a host build and is not there in
    /// the Flatpak the user runs.
    #[test]
    fn a_staged_forward_file_never_lands_in_the_sandboxs_private_tmp() {
        let directory = super::forward_staging_dir();

        assert!(
            directory.starts_with(gtk::glib::user_cache_dir()),
            "a staged forward file belongs under the app's cache directory: {directory:?}"
        );
        assert!(
            !directory.starts_with(std::env::temp_dir()),
            "and never under /tmp, which a sandbox does not share: {directory:?}"
        );
    }
}
