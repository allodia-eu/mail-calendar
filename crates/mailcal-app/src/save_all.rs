//! Saving every attachment of a message into a folder the reader chose, at once
//! (`docs/reading-actions.md`, "Saving every attachment").

use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
};

use engine_api::Provider;

use crate::{App, reference::MessageRef};

#[cfg(test)]
#[path = "tests_save_all.rs"]
mod tests;

/// How many numbered names a file tries before the save gives up, so a folder that somehow
/// holds every one of them ends the loop rather than spinning.
const MAX_NUMBERED: u32 = 1000;

/// Whether the reading view offers Save all beside the attachments it lists.
///
/// Only from two files up: with one, the Save beside it already does exactly this, and a second
/// control for the same thing is one more to read past.
#[must_use]
pub fn offers_save_all(attachment_count: usize) -> bool {
    attachment_count >= 2
}

impl<P: Provider> App<P> {
    /// Writes every attachment of `message` into `directory`, each under the sender's name made
    /// safe for a filesystem, and returns the paths written in the order the reading view lists
    /// them.
    ///
    /// Nothing already in the folder is written over: a name that is taken, by a file that was
    /// there or by an earlier attachment of the same name, is numbered (`scan (2).pdf`), as a
    /// file manager names a second copy.
    ///
    /// All or nothing. Every file is decoded before the first is written, and a write that
    /// fails removes the files this call already wrote, so a folder never holds part of a
    /// message's files with nothing saying which part.
    ///
    /// # Errors
    ///
    /// A plain error string when the message or account cannot be resolved, the provider or
    /// cache read fails, or a file cannot be written.
    pub async fn save_all_attachments(
        &self,
        message: MessageRef,
        directory: &str,
    ) -> Result<Vec<String>, String> {
        let Some(original) = self.find_message_in(&message).await else {
            return Err("message is not available".to_owned());
        };
        let Some(acct) = self.account_handle(&message.account).await else {
            return Err("account is not connected".to_owned());
        };
        let Some(provider) = acct.providers.first() else {
            return Err("mail provider is not connected".to_owned());
        };
        let listed = self
            .engine
            .message_attachments(provider, &message.account, &original)
            .await
            .map_err(|err| err.to_string())?;
        let mut files = Vec::with_capacity(listed.len());
        for attachment in &listed {
            let content = self
                .engine
                .message_attachment(provider, &message.account, &original, attachment.id())
                .await
                .map_err(|err| err.to_string())?
                .ok_or_else(|| "attachment is not available".to_owned())?;
            files.push((
                mailcal_composer::safe_file_name(attachment.file_name(), ""),
                content,
            ));
        }
        let directory = Path::new(directory);
        let mut written: Vec<PathBuf> = Vec::with_capacity(files.len());
        for (name, content) in &files {
            match create_under_free_name(directory, name, content.bytes()) {
                Ok(path) => written.push(path),
                Err(err) => {
                    for path in &written {
                        let _ = std::fs::remove_file(path);
                    }
                    return Err(err.to_string());
                }
            }
        }
        log::info!("reading: saved {} attachment(s) to a folder", written.len());
        Ok(written
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect())
    }
}

/// Creates `name` in `directory`, or the first numbered name that is free, and writes `bytes`
/// into it. Created exclusively, so a file that appears between the check and the write is
/// never truncated.
fn create_under_free_name(directory: &Path, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    for n in 1..=MAX_NUMBERED {
        let path = directory.join(if n == 1 {
            name.to_owned()
        } else {
            numbered(name, n)
        });
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                return match file.write_all(bytes) {
                    Ok(()) => Ok(path),
                    Err(err) => {
                        drop(file);
                        let _ = std::fs::remove_file(&path);
                        Err(err)
                    }
                };
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "every numbered name is taken",
    ))
}

/// `name` as the `n`th copy: the number goes before the extension, so the file still opens in
/// what opens the first.
fn numbered(name: &str, n: u32) -> String {
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => format!("{stem} ({n}).{extension}"),
        _ => format!("{name} ({n})"),
    }
}
