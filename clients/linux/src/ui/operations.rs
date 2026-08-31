//! Attachment and submission operations that cross the GTK/core boundary.

use std::{
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use gtk::gio;
use mailcal_bindings::{ComposerFileAttachment, MailcalApp, Recipients};

use super::{
    AppInput, AppModel,
    composer_model::{ComposeKind, ComposerSubmission, PickedFile},
    web_security::safe_extension,
};
use crate::l10n;

impl AppModel {
    pub(super) fn submit_composer(&mut self, submission: &ComposerSubmission) {
        let Some(app) = &self.app else {
            return;
        };
        if submit(app, submission).is_ok() {
            self.composer = None;
            self.composer_error = false;
        } else {
            self.composer_error = true;
        }
    }

    pub(super) fn save_attachment(
        &self,
        id: u32,
        destination: PathBuf,
        sender: relm4::Sender<AppInput>,
    ) {
        let (Some(app), Some(opened)) = (&self.app, &self.reading.opened) else {
            return;
        };
        let app = Arc::clone(app);
        let account = opened.account.clone();
        let key = opened.key.clone();
        std::thread::spawn(move || {
            let destination_existed = destination.exists();
            let saved = app
                .save_attachment(account, key, id, destination.to_string_lossy().into_owned())
                .is_ok();
            if !saved && !destination_existed {
                let _ = std::fs::remove_file(destination);
            }
            sender.emit(AppInput::AttachmentSaved(saved));
        });
    }

    pub(super) fn open_attachment(
        &self,
        id: u32,
        file_name: &str,
        sender: relm4::Sender<AppInput>,
    ) {
        let (Some(app), Some(opened)) = (&self.app, &self.reading.opened) else {
            return;
        };
        let app = Arc::clone(app);
        let account = opened.account.clone();
        let key = opened.key.clone();
        let extension = safe_extension(file_name);
        std::thread::spawn(move || {
            let directory = opened_attachment_dir();
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos());
            let path = directory.join(format!("attachment-{nonce}{extension}"));
            let result = std::fs::create_dir_all(&directory)
                .map_err(|_| ())
                .and_then(|()| {
                    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
                        .map_err(|_| ())
                })
                .and_then(|()| {
                    app.save_attachment(account, key, id, path.to_string_lossy().into_owned())
                        .map_err(|_| ())
                })
                .and_then(|()| {
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                        .map_err(|_| ())
                });
            if result.is_err() {
                let _ = std::fs::remove_file(&path);
            }
            sender.emit(AppInput::AttachmentDecoded(result.map(|()| path)));
        });
    }

    /// Hands a decoded attachment to whatever the desktop opens that type with.
    ///
    /// **`GtkFileLauncher`, not a `file://` URI through `AppInfo`.** A sandboxed build has no
    /// host application database to resolve the type against, and the path is one inside its own
    /// filesystem view; so the hand-off has to go through the OpenURI portal, which takes a file
    /// descriptor rather than a path. `GtkFileLauncher` is that, and it is asynchronous, so a
    /// portal that is slow to answer cannot freeze the window the attachment was opened from.
    ///
    /// The failure therefore arrives as its own input rather than a return value.
    ///
    /// **The app chooser appearing for the first few attachments is the portal's, not ours.**
    /// `always-ask` is left at its default of false, so we ask for the default handler: but
    /// xdg-desktop-portal shows the chooser to a *sandboxed* caller until the same app has been
    /// picked three times, then uses it silently (`flatpak permissions desktop-used-apps` records
    /// `<app>,<count>,<threshold>`). It is consent design, not a setting to defeat.
    pub(super) fn launch_attachment(
        &mut self,
        result: Result<PathBuf, ()>,
        sender: relm4::Sender<AppInput>,
    ) {
        let Ok(path) = result else {
            self.notice = Some(l10n::attachment_open_failed().to_owned());
            return;
        };
        let file = gio::File::for_path(path);
        gtk::FileLauncher::new(Some(&file)).launch(
            None::<&gtk::Window>,
            gio::Cancellable::NONE,
            move |outcome| {
                if let Err(error) = outcome {
                    // The message is the portal's; it names no file and no address.
                    log::warn!("an attachment could not be opened: {error}");
                    sender.emit(AppInput::AttachmentOpenFailed);
                }
            },
        );
    }
}

/// Where a decoded attachment is written before the desktop is asked to open it.
///
/// **The app's own cache directory, never `std::env::temp_dir()`.** Inside a Flatpak `/tmp` is the
/// sandbox's private tmpfs: the host cannot see it, and neither can the application the portal
/// launches: so the hand-off fails with nothing more than "The application launch failed", which
/// reads as the attachment being broken rather than as being in the wrong place. The cache
/// directory is a real host path (`~/.var/app/<app-id>/cache` in a Flatpak), so a descriptor taken
/// from it is one the portal can pass on.
///
/// Keyed by process id, like the temporary directory it replaces, so two running copies never
/// collide; the directory is 0700 and the file 0600, because a decoded attachment is the user's
/// mail sitting on disk.
fn opened_attachment_dir() -> PathBuf {
    gtk::glib::user_cache_dir()
        .join("mailcal")
        .join(format!("opened-{}", std::process::id()))
}

fn submit(
    app: &MailcalApp,
    submission: &ComposerSubmission,
) -> Result<(), mailcal_bindings::MailcalError> {
    let recipients = Recipients {
        to: submission.to.clone(),
        cc: submission.cc.clone(),
        bcc: submission.bcc.clone(),
    };
    let files = submission
        .files
        .iter()
        .map(file_attachment)
        .collect::<Vec<_>>();
    match submission.request.kind {
        ComposeKind::New => app.submit_rich_mail_with_files(
            recipients,
            submission.subject.clone(),
            submission.document_json.clone(),
            files,
            submission.from.clone(),
        ),
        ComposeKind::Reply | ComposeKind::ReplyAll => app.submit_rich_reply_with_files(
            submission.request.account.clone().unwrap_or_default(),
            submission.request.key.clone().unwrap_or_default(),
            recipients,
            submission.document_json.clone(),
            files,
            submission.from.clone(),
        ),
        ComposeKind::Forward => app.submit_rich_forward_with_files(
            submission.request.account.clone().unwrap_or_default(),
            submission.request.key.clone().unwrap_or_default(),
            recipients,
            submission.document_json.clone(),
            files,
            submission.from.clone(),
        ),
    }
}

fn file_attachment(file: &PickedFile) -> ComposerFileAttachment {
    ComposerFileAttachment {
        path: file.path.clone(),
        file_name: file.file_name.clone(),
        media_type: file.media_type.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use mailcal_bindings::{SendStatus, SnapshotRow};

    use super::opened_attachment_dir;

    /// A decoded attachment has to live where the portal can hand it on.
    ///
    /// `/tmp` inside a Flatpak is the sandbox's own tmpfs: invisible to the host and to whatever
    /// application the portal launches, which answers "The application launch failed" and looks to
    /// the user like a broken attachment. The cache directory is a real host path.
    #[test]
    fn a_decoded_attachment_never_lands_in_the_sandboxs_private_tmp() {
        let directory = opened_attachment_dir();

        assert!(
            directory.starts_with(gtk::glib::user_cache_dir()),
            "an opened attachment belongs under the app's cache directory: {directory:?}"
        );
        assert!(
            !directory.starts_with(std::env::temp_dir()),
            "and never under /tmp, which a sandbox does not share: {directory:?}"
        );
    }

    use super::{ComposeKind, ComposerSubmission, submit};
    use crate::{boot, observer::SurfaceObserver, ui::composer_model::ComposeRequest};

    const DOCUMENT: &str = r#"{
        "blocks": [{
            "Paragraph": {
                "content": [{
                    "Text": {
                        "text": "Linux harness reply",
                        "bold": false,
                        "italic": false,
                        "underline": false
                    }
                }]
            }
        }],
        "attachments": []
    }"#;

    #[test]
    #[ignore = "requires MAILCAL_DEV_ACCOUNT=stalwart and the local harness"]
    fn linux_reply_reaches_stalwart_through_the_shared_submission_path() {
        assert_eq!(
            std::env::var("MAILCAL_DEV_ACCOUNT").as_deref(),
            Ok("stalwart"),
            "run with MAILCAL_DEV_ACCOUNT=stalwart and scripts/dev/harness.sh up",
        );
        let (sender, _receiver) = relm4::channel();
        let app = boot::app(Box::new(SurfaceObserver::new(sender)))
            .expect("boot harness account")
            .app;
        let deadline = Instant::now() + Duration::from_secs(20);
        let (account, key) = loop {
            let snapshot = app.mailbox_list();
            if let Some(row) = snapshot.rows.first() {
                break match row {
                    SnapshotRow::Flat { row } => (row.account.clone(), row.key.clone()),
                    SnapshotRow::Thread { row } => (row.account.clone(), row.latest_key.clone()),
                };
            }
            assert!(
                Instant::now() < deadline,
                "harness mailbox did not materialize"
            );
            std::thread::sleep(Duration::from_millis(100));
        };
        let recipients = app.reply_recipients(account.clone(), key.clone(), false);
        let submission = ComposerSubmission {
            request: ComposeRequest {
                kind: ComposeKind::Reply,
                account: Some(account.clone()),
                key: Some(key),
                initial_to: recipients.to.clone(),
                initial_cc: recipients.cc.clone(),
                initial_bcc: String::new(),
                subject: String::new(),
                initial_body: None,
                quote: None,
                initial_from: Some(account.clone()),
                seeds_signature: true,
            },
            to: recipients.to,
            cc: recipients.cc,
            bcc: String::new(),
            subject: String::new(),
            document_json: DOCUMENT.to_owned(),
            files: Vec::new(),
            from: Some(account),
        };

        submit(&app, &submission).expect("schedule harness reply");
        loop {
            match app.send_status() {
                SendStatus::Sent => break,
                // The reply went out either way; against the harness a missing Sent copy is
                // itself a defect, so this fails rather than passing quietly.
                SendStatus::SentNotFiled => panic!("harness reply sent but its copy was not filed"),
                SendStatus::Failed => panic!("harness reply failed"),
                SendStatus::Idle | SendStatus::Sending => {}
            }
            assert!(Instant::now() < deadline, "harness reply did not finish");
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
