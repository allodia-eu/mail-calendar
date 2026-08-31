//! Rich composer submission helpers for host-selected file attachments.

use std::{collections::HashSet, sync::Arc};

use mailcal_app::{ComposerBlob as AppComposerBlob, Intent as AppIntent};
use mailcal_composer::{
    AttachmentDisposition, AttachmentId, ComposerDocument, DraftAttachment as ComposerAttachment,
    DraftBlobHandle,
};

use crate::{
    MailcalApp, MailcalError, Recipients,
    composer::{message_ref, send_account},
};

/// A host-selected file to attach to a rich composer submission.
#[derive(uniffi::Record)]
pub struct ComposerFileAttachment {
    /// Filesystem path the host selected or staged.
    pub path: String,
    /// Suggested filename for the outgoing MIME part.
    pub file_name: String,
    /// Media type such as `application/pdf`; blank falls back to
    /// `application/octet-stream`.
    pub media_type: String,
}

/// One selected file staged for the async read: the blob handle its bytes will fill and the
/// filesystem path to read them from. Held between synchronous validation and the spawned
/// send so the (potentially large) read stays off the host's calling thread.
#[derive(Debug)]
struct PendingFile {
    handle: DraftBlobHandle,
    path: String,
}

/// The validated document plus the files still to be read: the output of the synchronous
/// preparation step, consumed by the spawned send.
#[derive(Debug)]
struct PreparedFiles {
    document: ComposerDocument,
    pending: Vec<PendingFile>,
}

#[uniffi::export]
impl MailcalApp {
    /// Sends a rich composer document with regular file attachments read from host-selected
    /// paths. The document is parsed and validated synchronously; the attachment bytes are
    /// read in Rust on the internal runtime (never on the host's calling thread) and do not
    /// cross FFI.
    ///
    /// `from` names the sending account (the composer's From dropdown), as in
    /// [`MailcalApp::submit_rich_mail`]; omit it to let the core derive it.
    #[uniffi::method(default(from = None))]
    pub fn submit_rich_mail_with_files(
        &self,
        recipients: Recipients,
        subject: String,
        document_json: String,
        files: Vec<ComposerFileAttachment>,
        from: Option<String>,
    ) -> Result<(), MailcalError> {
        let prepared = prepare_with_files(&document_json, files)?;
        let from = send_account(from)?;
        let Recipients { to, cc, bcc } = recipients;
        self.spawn_with_files(prepared, move |document, blobs| AppIntent::SubmitRichMail {
            from,
            to,
            cc,
            bcc,
            subject,
            document,
            blobs,
        });
        Ok(())
    }

    /// Replies with a rich composer document plus regular file attachments. `from` names the
    /// sending account (the composer's From dropdown); omit it to reply from `account`.
    #[uniffi::method(default(from = None))]
    pub fn submit_rich_reply_with_files(
        &self,
        account: String,
        key: String,
        recipients: Recipients,
        document_json: String,
        files: Vec<ComposerFileAttachment>,
        from: Option<String>,
    ) -> Result<(), MailcalError> {
        let message = message_ref(&account, key)?;
        let prepared = prepare_with_files(&document_json, files)?;
        let from = send_account(from)?;
        let Recipients { to, cc, bcc } = recipients;
        self.spawn_with_files(prepared, move |document, blobs| {
            AppIntent::SubmitRichReply {
                message,
                from,
                to,
                cc,
                bcc,
                document,
                blobs,
            }
        });
        Ok(())
    }

    /// Forwards with a rich composer document plus regular file attachments. `from` names the
    /// sending account (the composer's From dropdown); omit it to forward from `account`.
    #[uniffi::method(default(from = None))]
    pub fn submit_rich_forward_with_files(
        &self,
        account: String,
        key: String,
        recipients: Recipients,
        document_json: String,
        files: Vec<ComposerFileAttachment>,
        from: Option<String>,
    ) -> Result<(), MailcalError> {
        let message = message_ref(&account, key)?;
        let prepared = prepare_with_files(&document_json, files)?;
        let from = send_account(from)?;
        let Recipients { to, cc, bcc } = recipients;
        self.spawn_with_files(prepared, move |document, blobs| {
            AppIntent::SubmitRichForward {
                message,
                from,
                to,
                cc,
                bcc,
                document,
                blobs,
            }
        });
        Ok(())
    }
}

impl MailcalApp {
    /// Reads the prepared files off the calling thread, then dispatches the intent the
    /// `make_intent` closure builds from the document and resolved blobs. Fire-and-forget: a
    /// file that can't be read after validation (a rare mid-flight removal) is logged and the
    /// send is dropped rather than sending a draft with missing bytes.
    fn spawn_with_files<F>(&self, prepared: PreparedFiles, make_intent: F)
    where
        F: FnOnce(ComposerDocument, Vec<AppComposerBlob>) -> AppIntent + Send + 'static,
    {
        let app = Arc::clone(&self.app);
        let PreparedFiles { document, pending } = prepared;
        self.runtime.spawn(async move {
            match read_pending(pending) {
                Ok(blobs) => app.dispatch(make_intent(document, blobs)).await,
                Err(err) => {
                    log::warn!("composer attachment read failed: {err}");
                }
            }
        });
    }
}

/// Reads every pending file's bytes into its blob. Runs on the internal runtime, not the
/// host's calling thread.
fn read_pending(pending: Vec<PendingFile>) -> Result<Vec<AppComposerBlob>, std::io::Error> {
    pending
        .into_iter()
        .map(|file| std::fs::read(&file.path).map(|bytes| AppComposerBlob::new(file.handle, bytes)))
        .collect()
}

/// Parses and validates the composer document and appends the selected files as regular
/// attachments, staging each for a later off-thread read. Only cheap metadata (file size) is
/// read here, so this stays fast enough for the host's UI thread; the byte read is deferred.
///
/// Every attachment the rendered document references must be one of the supplied files;
/// a document carrying a pre-existing attachment or inline-image whose bytes this call does
/// not provide is rejected synchronously (rather than dispatched and failed asynchronously).
fn prepare_with_files(
    document_json: &str,
    files: Vec<ComposerFileAttachment>,
) -> Result<PreparedFiles, MailcalError> {
    let mut document: ComposerDocument = serde_json::from_str(document_json)
        .map_err(|err| MailcalError::Composer(format!("invalid composer document JSON: {err}")))?;
    let mut used_ids = document
        .attachments
        .iter()
        .map(|attachment| attachment.id.as_str().to_owned())
        .collect::<HashSet<_>>();
    let mut used_handles = document
        .attachments
        .iter()
        .map(|attachment| attachment.blob.as_str().to_owned())
        .collect::<HashSet<_>>();
    let mut pending = Vec::with_capacity(files.len());
    for file in files {
        // Cheap stat only: the actual read happens off-thread in `read_pending`.
        let size = std::fs::metadata(&file.path)
            .map_err(|err| {
                MailcalError::Composer(format!("cannot read selected attachment: {err}"))
            })?
            .len();
        let id_value = unique_value("native-file", &mut used_ids);
        let handle_value = unique_value("native-file", &mut used_handles);
        let id = AttachmentId::new(id_value)
            .ok_or_else(|| MailcalError::Composer("attachment id is blank".to_owned()))?;
        let handle = DraftBlobHandle::new(handle_value)
            .ok_or_else(|| MailcalError::Composer("attachment blob handle is blank".to_owned()))?;
        let file_name = safe_file_name(&file);
        let media_type = safe_media_type(&file.media_type);
        document.attachments.push(ComposerAttachment {
            id,
            blob: handle.clone(),
            file_name,
            media_type,
            size: Some(size),
            disposition: AttachmentDisposition::Attachment,
        });
        pending.push(PendingFile {
            handle,
            path: file.path,
        });
    }
    let output = mailcal_composer::render(&document)
        .map_err(|err| MailcalError::Composer(err.to_string()))?;
    // Reject a document that references bytes this call does not supply: the file-attachment
    // path only fills the files it appends, so any other referenced blob would send empty.
    let supplied = pending
        .iter()
        .map(|file| file.handle.as_str())
        .collect::<HashSet<_>>();
    for attachment in output
        .inline_attachments
        .iter()
        .chain(output.attachments.iter())
    {
        if !supplied.contains(attachment.blob.as_str()) {
            return Err(MailcalError::Composer(format!(
                "missing bytes for composer blob {}",
                attachment.blob.as_str()
            )));
        }
    }
    Ok(PreparedFiles { document, pending })
}

fn unique_value(prefix: &str, used: &mut HashSet<String>) -> String {
    let mut index = 0;
    loop {
        let value = format!("{prefix}-{index}");
        if used.insert(value.clone()) {
            return value;
        }
        index += 1;
    }
}

fn safe_file_name(file: &ComposerFileAttachment) -> String {
    let candidate = if file.file_name.trim().is_empty() {
        std::path::Path::new(&file.path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("attachment")
    } else {
        file.file_name.as_str()
    };
    let cleaned = candidate
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>();
    let cleaned = cleaned.trim_matches(['.', ' ', '_']);
    if cleaned.is_empty() {
        "attachment".to_owned()
    } else {
        cleaned.to_owned()
    }
}

/// Normalises a host-reported media type to a well-formed `type/subtype`, falling back to
/// `application/octet-stream` for anything malformed. Requires exactly one `/` with a
/// non-empty type and subtype drawn from the RFC token-ish charset: so degenerate values
/// like `/`, `a/`, `/b`, or `a/b/c` never reach the outgoing `Content-Type`.
fn safe_media_type(value: &str) -> String {
    let trimmed = value.trim();
    let valid_token = |token: &str| {
        !token.is_empty()
            && token
                .bytes()
                .all(|b| matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'+' | b'-' | b'.'))
    };
    let mut parts = trimmed.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(ty), Some(sub), None) if valid_token(ty) && valid_token(sub) => {
            trimmed.to_ascii_lowercase()
        }
        _ => "application/octet-stream".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(name: &str, bytes: &[u8]) -> String {
        let path = std::env::temp_dir().join(format!(
            "mailcal-compose-attachment-{}-{name}",
            std::process::id()
        ));
        std::fs::write(&path, bytes).expect("seed attachment");
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn prepare_with_files_appends_regular_attachment_and_sanitizes_metadata() {
        let path = seed("append.txt", b"hello");
        let prepared = prepare_with_files(
            r#"{"blocks":[],"attachments":[]}"#,
            vec![ComposerFileAttachment {
                path: path.clone(),
                file_name: r#"..\hello?.txt"#.to_owned(),
                media_type: "bad media".to_owned(),
            }],
        )
        .expect("prepared");
        assert_eq!(prepared.document.attachments.len(), 1);
        let attachment = &prepared.document.attachments[0];
        assert_eq!(attachment.id.as_str(), "native-file-0");
        assert_eq!(attachment.file_name, "hello_.txt");
        assert_eq!(attachment.media_type, "application/octet-stream");
        assert_eq!(attachment.size, Some(5));
        assert!(matches!(
            attachment.disposition,
            AttachmentDisposition::Attachment
        ));
        // The bytes are read off-thread; the pending list carries the path until then.
        let blobs = read_pending(prepared.pending).expect("read bytes");
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].bytes, b"hello");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn prepare_with_files_assigns_unique_ids_across_multiple_files() {
        let first = seed("first.txt", b"one");
        let second = seed("second.txt", b"two");
        let prepared = prepare_with_files(
            r#"{"blocks":[],"attachments":[]}"#,
            vec![
                ComposerFileAttachment {
                    path: first.clone(),
                    file_name: "first.txt".to_owned(),
                    media_type: "text/plain".to_owned(),
                },
                ComposerFileAttachment {
                    path: second.clone(),
                    file_name: "second.txt".to_owned(),
                    media_type: "text/plain".to_owned(),
                },
            ],
        )
        .expect("prepared");
        assert_eq!(prepared.document.attachments.len(), 2);
        assert_eq!(
            prepared.document.attachments[0].id.as_str(),
            "native-file-0"
        );
        assert_eq!(
            prepared.document.attachments[1].id.as_str(),
            "native-file-1"
        );
        let blobs = read_pending(prepared.pending).expect("read bytes");
        assert_eq!(blobs.len(), 2);
        assert_eq!(blobs[0].bytes, b"one");
        assert_eq!(blobs[1].bytes, b"two");
        let _ = std::fs::remove_file(first);
        let _ = std::fs::remove_file(second);
    }

    #[test]
    fn prepare_with_files_rejects_a_document_referencing_unsupplied_bytes() {
        let path = seed("reject.txt", b"hello");
        // A pre-existing attachment whose bytes this call cannot supply; must be rejected
        // synchronously rather than dispatched and failed asynchronously.
        let err = prepare_with_files(
            r#"{"blocks":[],"attachments":[{"id":"ghost","blob":"ghost","file_name":"ghost.pdf","media_type":"application/pdf","size":0,"disposition":"Attachment"}]}"#,
            vec![ComposerFileAttachment {
                path: path.clone(),
                file_name: "reject.txt".to_owned(),
                media_type: "text/plain".to_owned(),
            }],
        )
        .expect_err("must reject unsupplied blob");
        assert!(matches!(err, MailcalError::Composer(msg) if msg.contains("ghost")));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn safe_media_type_rejects_degenerate_values() {
        assert_eq!(safe_media_type("application/pdf"), "application/pdf");
        assert_eq!(safe_media_type("image/SVG+xml"), "image/svg+xml");
        for bad in ["/", "a/", "/b", "a/b/c", "bad media", "", "text"] {
            assert_eq!(
                safe_media_type(bad),
                "application/octet-stream",
                "expected fallback for {bad:?}"
            );
        }
    }
}
