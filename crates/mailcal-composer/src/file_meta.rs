//! Attachment metadata a host hands us, made safe to put in a MIME header.
//!
//! Both routes a file reaches the composer by arrive here: the user picking one in the composer's
//! own file dialog, and another app sharing one into the app ([`crate::share`]). Neither name nor
//! media type is ours: the first comes from a filesystem, the second from whatever the sharing app
//! chose to say, and on Android that is routinely `*/*`. So both are treated as hostile input and
//! normalised once, here, rather than per platform.
//!
//! What the normalisation is actually defending, in order of how badly each fails:
//!
//! - **A header break.** A name carrying CR or LF ends the `Content-Disposition` line and starts
//!   whatever the sender wrote next. Every control character is dropped.
//! - **A path escaping the attachment.** A shared name may be a whole path (`../../etc/passwd`,
//!   `photos\holiday.jpg`); only its final component is an attachment name, so that is what is
//!   kept.
//! - **A name that reads as a different file than it is.** A right-to-left override renders
//!   `holiday\u{202E}gpj.exe` as `holidayexe.jpg` in the recipient's list, which is the oldest
//!   attachment trick there is. The bidirectional formatting characters are dropped, so the name
//!   reads in the order its bytes are in.

/// The longest attachment name we emit, in bytes.
///
/// Under the 255 every common filesystem stops at, with room for the recipient's client to add
/// its own ` (2)` when a name collides. A longer name is truncated **through its stem**, so the
/// extension survives: an extension is what decides which application opens the file, and losing
/// it costs the recipient more than losing the middle of a long name.
const MAX_FILE_NAME_BYTES: usize = 200;

/// The name to put on an attachment, given whatever the host suggested and the path it staged.
///
/// `suggested` wins when it holds anything; a host that knows a display name (a share's own
/// title, a content provider's `DISPLAY_NAME`) knows better than the temporary file it staged
/// the bytes into. Otherwise the path's final component is used, and an empty result becomes
/// `attachment` rather than an unnamed part.
#[must_use]
pub fn safe_file_name(suggested: &str, path: &str) -> String {
    let candidate = if suggested.trim().is_empty() {
        path
    } else {
        suggested
    };
    let cleaned: String = base_name(candidate)
        .chars()
        // Reserved punctuation becomes `_`, so the name still shows something was there.
        // Invisible characters are dropped outright: replacing one with `_` would add visible
        // noise standing for something the user never saw in the first place.
        .filter_map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => Some('_'),
            ch if ch.is_control() || is_bidi_control(ch) => None,
            ch => Some(ch),
        })
        .collect();
    let trimmed = cleaned.trim_matches(['.', ' ', '_']);
    if trimmed.is_empty() {
        "attachment".to_owned()
    } else {
        truncate_keeping_extension(trimmed)
    }
}

/// The media type to put on an attachment, given what the host declared and the resolved name.
///
/// A declared type is used when it is a well-formed `type/subtype`; its parameters are dropped
/// (`text/plain; charset=utf-8` attaches as `text/plain`), since a parameter never has to survive
/// to make the part readable and every one of them is another string reaching a header. Anything
/// malformed, wildcarded, or absent falls back to the extension, and then to
/// `application/octet-stream`, which is always a truthful answer: it says only "bytes".
#[must_use]
pub fn safe_media_type(declared: &str, file_name: &str) -> String {
    declared_type(declared)
        .or_else(|| extension_type(file_name).map(str::to_owned))
        .unwrap_or_else(|| "application/octet-stream".to_owned())
}

/// The final component of a path, treating both separators as one: a name arriving from a
/// Windows share reaches a Linux core with backslashes still in it, and neither side's
/// separator is a legal character in the name we emit.
fn base_name(value: &str) -> &str {
    value.rsplit(['/', '\\']).next().unwrap_or(value)
}

/// Whether `ch` is a bidirectional formatting character: the embeddings and overrides
/// (U+202A–U+202E) and the isolates (U+2066–U+2069). Each reorders the characters after it
/// without being visible itself, which is what makes a name readable as a different file
/// than it is.
const fn is_bidi_control(ch: char) -> bool {
    matches!(ch, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Truncates to [`MAX_FILE_NAME_BYTES`], taking the bytes out of the stem so the extension
/// survives. An "extension" is only recognised when it is short and last: a dot in the middle
/// of a long name is part of the name.
fn truncate_keeping_extension(name: &str) -> String {
    if name.len() <= MAX_FILE_NAME_BYTES {
        return name.to_owned();
    }
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .filter(|extension| !extension.is_empty() && extension.len() <= 16)
        .map_or(String::new(), |extension| format!(".{extension}"));
    let mut keep = MAX_FILE_NAME_BYTES.saturating_sub(extension.len());
    // Never split a character in half: walk back to the nearest boundary.
    while keep > 0 && !name.is_char_boundary(keep) {
        keep -= 1;
    }
    format!("{}{extension}", &name[..keep])
}

/// A host-declared media type, lowercased and stripped of its parameters, or `None` when it is
/// not a well-formed `type/subtype`.
///
/// Wildcards are refused explicitly. `*/*` is what Android hands a share target that accepts
/// anything, and it is not a media type: putting it on the part would tell the recipient's
/// client nothing while looking like it had been told something.
fn declared_type(declared: &str) -> Option<String> {
    let essence = declared.split(';').next().unwrap_or(declared).trim();
    let valid_token = |token: &str| {
        !token.is_empty()
            && token != "*"
            && token
                .bytes()
                .all(|b| matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'+' | b'-' | b'.'))
    };
    let mut parts = essence.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(ty), Some(sub), None) if valid_token(ty) && valid_token(sub) => {
            Some(essence.to_ascii_lowercase())
        }
        _ => None,
    }
}

/// The media type a file name's extension implies.
///
/// Deliberately a short table of what people actually attach, not a copy of the IANA registry:
/// anything missing falls back to `application/octet-stream`, which costs the recipient a
/// generic icon and nothing else. Sniffing the bytes would be the alternative and is a worse
/// trade, it means reading every shared file before the composer can open.
fn extension_type(file_name: &str) -> Option<&'static str> {
    let extension = file_name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match extension.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "txt" | "log" => "text/plain",
        "md" => "text/markdown",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "tar" => "application/x-tar",
        "7z" => "application/x-7z-compressed",
        "rtf" => "application/rtf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "odp" => "application/vnd.oasis.opendocument.presentation",
        "epub" => "application/epub+zip",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "ics" => "text/calendar",
        "eml" => "message/rfc822",
        "vcf" => "text/vcard",
        _ => return None,
    })
}
