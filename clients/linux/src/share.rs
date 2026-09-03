//! Desktop share ingress: files the desktop, or a local process, hands this app to put in a
//! message. Naming, typing and the caps stay in the shared core (`docs/os-integration.md`).
//!
//! There is no share portal on Linux, so the two channels a desktop actually offers are both
//! command lines, which is what `Exec=mailcal %U` and the `MimeType=` set in the generated desktop
//! entry are for:
//!
//! - **"Open With"**, which passes `file://` URIs for whatever the user selected;
//! - **`--attach <path>`**, for a script or a file manager's own "send by email" action.
//!
//! Both are the user naming this app for these files, which is what makes them admissible under
//! the rule that attachments never arrive from a URI: a `mailto:` link, wherever it came from,
//! cannot carry one.

use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

use gtk::{gio, prelude::FileExt};
use mailcal_bindings::{SharePrefill, ShareRequest, SharedFile, prefill_from_share};

/// How a local process attaches a file explicitly.
const ATTACH_FLAG: &str = "--attach";

/// The media type the desktop's own content database gives a file.
///
/// Shared with the composer's file picker so a file attached by "Open With" and the same file
/// attached from the dialog are typed identically. It is offered to the core as the host's
/// *declared* type, which the core still validates and may overrule, so this widens what Linux
/// can recognise without moving where the decision is made.
pub(crate) fn media_type_for(file_name: &str) -> String {
    let (content_type, _) = gio::content_type_guess(Some(Path::new(file_name)), None);
    gio::content_type_get_mime_type(&content_type)
        .map_or_else(String::new, |value| value.to_string())
}

/// The share a command line carries, or `None` when it names no readable file.
///
/// `arguments` is the whole argv, argv[0] included, as GApplication's `command-line` signal hands
/// it over. A mail link is **not** a share and is left for [`crate::mail_link`]: `main` asks that
/// question first, and this one only sees what it declined.
pub(crate) fn prefill_arguments(arguments: &[OsString]) -> Option<SharePrefill> {
    let mut files = Vec::new();
    let mut arguments = arguments.iter().skip(1);
    while let Some(argument) = arguments.next() {
        let path = if argument == OsStr::new(ATTACH_FLAG) {
            // `--attach` names the next argument whatever it looks like, so a file called `-x`
            // is still attachable and a missing value simply ends the walk.
            arguments.next().map(OsString::as_os_str)
        } else if argument.as_encoded_bytes().starts_with(b"-") {
            // Every other flag belongs to somebody else, and **only the flag is stepped over**,
            // never the token after it. Swallowing that token would be a guess about a flag this
            // module does not know, and the guess costs the user a file: the day something adds a
            // boolean `--calendar` here, as the Windows client already has, `mailcal --calendar
            // report.pdf` would silently drop the report.
            //
            // The other way round is the cheaper mistake. An unknown flag's *value* is considered
            // as a path, and reaches the composer only if it names a file that exists, so
            // `--poll-interval 30` attaches nothing. It would take an unknown flag whose value
            // happens to name a real file in the process's own directory, which is why a flag
            // added here should take its value as `--flag=value`.
            continue;
        } else {
            Some(argument.as_os_str())
        };
        if let Some(file) = path.and_then(readable_file) {
            files.push(file);
        }
    }

    if files.is_empty() {
        return None;
    }
    // Files only: a command line carries no shared text, and a `mailto:` argument never reaches
    // here, so there is nothing that could pre-fill a recipient.
    Some(prefill_from_share(ShareRequest {
        files,
        text: String::new(),
        subject: String::new(),
    }))
}

/// One argument as a file the core can be handed, or `None` when it does not name one.
///
/// A `file://` URI (what `%U` delivers) and a plain path are both accepted, and the result must
/// **exist and be a file**. That check is what keeps a stray argument, or a flag value this
/// module does not know about, from becoming an attachment with no bytes behind it: the failure
/// would otherwise surface at send, long after the composer claimed to hold it.
fn readable_file(argument: &OsStr) -> Option<SharedFile> {
    let path = if argument.as_encoded_bytes().starts_with(b"file://") {
        gio::File::for_uri(&argument.to_string_lossy()).path()?
    } else {
        Path::new(argument).to_path_buf()
    };
    if !path.is_file() {
        // Counted, never named: an argument is the user's own filesystem (`docs/logging.md`).
        log::info!("share argument skipped: not a readable file");
        return None;
    }
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    Some(SharedFile {
        declared_media_type: media_type_for(&file_name),
        // Left blank: the core takes the path's own final component, which for a local file is
        // the name the user knows it by. A host only supplies this when it staged the bytes
        // somewhere the name would be lost.
        suggested_name: String::new(),
        path: path.to_string_lossy().into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::prefill_arguments;

    /// A real file to hand the parser, since it refuses anything it cannot stat.
    fn seed(name: &str) -> OsString {
        let path =
            std::env::temp_dir().join(format!("mailcal-share-{}-{name}", std::process::id()));
        std::fs::write(&path, b"bytes").expect("seed a shared file");
        path.into_os_string()
    }

    fn argv(rest: &[OsString]) -> Vec<OsString> {
        let mut arguments = vec![OsString::from("mailcal")];
        arguments.extend_from_slice(rest);
        arguments
    }

    #[test]
    fn an_open_with_launch_attaches_what_it_names() {
        let file = seed("open-with.pdf");
        let prefill = prefill_arguments(&argv(std::slice::from_ref(&file))).expect("a share");
        assert_eq!(prefill.attachments.len(), 1);
        assert_eq!(prefill.attachments[0].path, file.to_string_lossy());
        assert!(prefill.attachments[0].file_name.ends_with("open-with.pdf"));
        assert_eq!(prefill.attachments[0].media_type, "application/pdf");
    }

    #[test]
    fn a_file_uri_is_a_path_like_any_other() {
        // What `Exec=mailcal %U` actually delivers.
        let file = seed("uri.txt");
        let uri = OsString::from(format!("file://{}", file.to_string_lossy()));
        let prefill = prefill_arguments(&argv(&[uri])).expect("a share");
        assert_eq!(prefill.attachments[0].path, file.to_string_lossy());
    }

    #[test]
    fn the_attach_flag_takes_the_argument_after_it() {
        let file = seed("flagged.txt");
        let prefill =
            prefill_arguments(&argv(&[OsString::from("--attach"), file.clone()])).expect("a share");
        assert_eq!(prefill.attachments.len(), 1);
        assert_eq!(prefill.attachments[0].path, file.to_string_lossy());
    }

    #[test]
    fn several_files_keep_the_order_they_were_given_in() {
        let first = seed("a.txt");
        let second = seed("b.txt");
        let prefill = prefill_arguments(&argv(&[first, second])).expect("a share");
        assert_eq!(prefill.attachments.len(), 2);
        assert!(prefill.attachments[0].file_name.ends_with("a.txt"));
        assert!(prefill.attachments[1].file_name.ends_with("b.txt"));
    }

    #[test]
    fn a_command_line_naming_no_file_is_not_a_share() {
        // The ordinary launch, and the one that matters: `mailcal` on its own must not open a
        // composer.
        assert!(prefill_arguments(&argv(&[])).is_none());
        assert!(prefill_arguments(&argv(&[OsString::from("--calendar")])).is_none());
        assert!(prefill_arguments(&argv(&[OsString::from("/nowhere/absent.pdf")])).is_none());
    }

    #[test]
    fn a_mail_link_is_not_a_share() {
        // `main` asks the mail-link question first, but a link must not read as a file here
        // either: it is not one, and a share may never pre-fill a recipient.
        let arguments = argv(&[OsString::from("mailto:ada@example.test?subject=Hi")]);
        assert!(prefill_arguments(&arguments).is_none());
    }

    #[test]
    fn a_flag_never_swallows_the_file_after_it() {
        // The regression this guards: only `--attach` takes a value, so stepping over an unknown
        // flag *and* the token after it would drop the user's file the day a boolean flag is added
        // here, as the Windows client already has one. The file must survive the flag.
        let file = seed("after-flag.txt");
        let arguments = argv(&[OsString::from("--calendar"), file.clone()]);
        let prefill = prefill_arguments(&arguments).expect("a share");
        assert_eq!(prefill.attachments.len(), 1);
        assert_eq!(prefill.attachments[0].path, file.to_string_lossy());
    }

    #[test]
    fn an_unknown_flags_value_reaches_the_composer_only_if_it_names_a_file() {
        // The cost of the rule above, stated so it is a decision rather than a surprise: an
        // unknown flag's value is considered as a path. Almost none name a file, so almost none
        // attach anything, which is why a flag added here should take its value as `--flag=value`.
        let arguments = argv(&[OsString::from("--poll-interval"), OsString::from("30")]);
        assert!(prefill_arguments(&arguments).is_none());
    }
}
