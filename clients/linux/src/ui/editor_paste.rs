//! A picture pasted into one of the editors: the message body, and the Settings signature editor,
//! which is the same bundle in body-only mode and meets the same toolkit.
//!
//! ⚠️ **WebKitGTK hands the page no clipboard files.** On a paste its `DataTransfer` answers
//! `getData` for the text flavours and nothing else: `types`, `items` and `files` are all empty
//! whatever is on the clipboard, and `getData("image/png")` is the empty string (measured against
//! 2.52). So the shared bundle's paste handler, which takes a picture off `clipboardData` on every
//! other host, can never see one here, and Ctrl+V of a screenshot puts nothing in the message.
//!
//! Nor can the page rescue it afterwards. Letting WebKit's own paste run inserts
//! `<img src="blob:…">`, and the composer's CSP is `connect-src 'none'`, so `fetch` and
//! `XMLHttpRequest` on that blob both fail; only a canvas re-encode gets at the pixels, which
//! throws away the original bytes and turns a pasted photograph into a far larger PNG.
//!
//! The host therefore answers the question the drop already asks: it reads the clipboard itself,
//! has the core sniff the bytes, and hands the picture to the editor through
//! `insertComposerImage`, the seam a dropped picture already arrives by. **Only a paste that
//! actually carries a picture is taken.** Anything else stays the page's, so pasting text is
//! untouched, and so is the plain-text paste a Ctrl+Shift+V asks for.
//!
//! **A clipboard carries a picture two ways, and both are ordinary.** A screenshot tool or a
//! browser's Copy Image puts the pixels there, as `image/png` and friends. A file manager's Copy
//! puts a *reference* to the file instead, as `text/uri-list` and the portal's own types, with no
//! `image/*` in sight; that is what copying a picture out of Files looks like. Both end up in the
//! message: the pixels directly, the file through `composer_attach::accept_files`, which is the one
//! place that decides what a file handed to the composer is for. A **pasted** picture is not asked
//! about, unlike a dropped one, because a paste is aimed at the caret and has already said.

//! **The trail this leaves is `debug`, deliberately.** A paste that works is not an event anyone
//! needs in a support window, and the refusals that matter are already logged by the core, once,
//! where the size cap and the byte sniff live. What these lines answer is "why did a paste do
//! nothing on a desktop nobody can sit in front of", which is what raising the level to DEBUG is
//! for (`docs/logging.md`). Offered media types only, never what they hold.

use std::rc::Rc;

use gtk::{gdk, gio, glib, prelude::*};
use webkit6::{ContextMenuAction, ContextMenuItem, WebView, prelude::WebViewExt};

use super::composer_attach::AcceptFiles;
use crate::l10n;

/// The formats the core will sniff a picture out of, asked for in this order. The list mirrors
/// `mailcal_app::composer_image::raster_media_type`; a type missing here is one the core would
/// refuse anyway, and one missing there cannot reach the message however it is asked for.
const PICTURE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/// How much is read at a time out of the clipboard's stream.
const CHUNK_BYTES: usize = 64 * 1024;

/// A picture read off the clipboard: the bytes, and the type the clipboard declared them to be.
pub(crate) struct PastedPicture {
    pub(crate) bytes: Vec<u8>,
    pub(crate) media_type: String,
}

/// What that picture is handed to, `None` when the clipboard said it held one and it could not be
/// read.
///
/// **The bytes, not a finished `data:` URI**, because the rule that turns one into the other is the
/// editor's own and the two editors do not share it: a message body takes the core's raster set up
/// to the core's cap, and a signature takes far less, because a signature rides in every message
/// the account sends. Each builds its own, where its own error line is in scope.
pub(crate) type PastePicture = Rc<dyn Fn(Option<PastedPicture>)>;

/// The two answers a paste can need, together, because both routes into one (the chord and the
/// menu item) need both.
#[derive(Clone)]
pub(crate) struct PasteAnswer {
    /// Pixels on the clipboard: an inline picture at the caret.
    pub(crate) picture: PastePicture,
    /// Files on the clipboard: whatever a dropped file gets.
    pub(crate) files: AcceptFiles,
}

/// The message body's answer: the core's raster rule, then the picture at the caret.
///
/// The file name is left to the core: a picture on the clipboard has no name, and
/// `addCapturedImage` mints `image-<n>.<ext>` from the type it sniffed rather than from anything
/// the desktop claimed.
pub(super) fn paste_into_message(editor: &WebView, error: &gtk::Label) -> PastePicture {
    let editor = editor.clone();
    let error = error.clone();
    Rc::new(move |picture: Option<PastedPicture>| {
        let data_url = picture.and_then(|picture| {
            mailcal_bindings::composer_image_data_url_from_bytes(&picture.bytes).ok()
        });
        // Never silent: the user watched a picture not arrive, and the drop question answers the
        // same failure with the same line.
        let Some(url) = data_url else {
            error.set_text(l10n::compose_image_failed());
            error.set_visible(true);
            return;
        };
        let payload = serde_json::json!({ "data_url": url, "file_name": "" });
        editor.evaluate_javascript(
            &format!("window.insertComposerImage({payload});"),
            None,
            None,
            None::<&gio::Cancellable>,
            |result| match result {
                Ok(_) => log::debug!("paste: the message took the pasted picture"),
                Err(_) => log::debug!("paste: the message refused the pasted picture"),
            },
        );
    })
}

/// Takes Ctrl+V (and the X11 convention Shift+Insert) when the clipboard holds a picture.
///
/// ⚠️ **The controller goes on the editor's HOST, not on the view**, and in the **capture** phase.
/// WebKitGTK installs key controllers of its own when it builds a `WebView`, and controllers on one
/// widget run in the order they were added, so ours would be second and never see a chord WebKit
/// had already claimed. Capture runs from the toplevel down, so an ancestor's controller is ahead
/// of every one of the view's. The same trap the drop target is written around
/// (`composer_attach`), for the same reason.
///
/// `host` must contain the editor and nothing else that takes the caret: the chord is only ever
/// ours when the message body is what the user is typing in, and a Ctrl+V in Subject belongs to
/// Subject.
pub(super) fn install(host: &impl IsA<gtk::Widget>, editor: &WebView, answer: PasteAnswer) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let view = editor.clone();
    keys.connect_key_pressed(move |_, key, _, state| {
        if !is_paste_chord(key, state) {
            return glib::Propagation::Proceed;
        }
        log::debug!("paste: requested from the keyboard");
        if take_clipboard(&view, &answer) {
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    host.as_ref().add_controller(keys);
}

/// Whether this keystroke is a paste.
///
/// Ctrl+**Shift**+V is deliberately not one: it is the paste-as-plain-text every toolkit binds, and
/// a user asking for plain text is not asking for the picture beside it.
fn is_paste_chord(key: gdk::Key, state: gdk::ModifierType) -> bool {
    let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
    let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
    let alt = state.contains(gdk::ModifierType::ALT_MASK);
    if alt {
        return false;
    }
    (ctrl && !shift && matches!(key, gdk::Key::v | gdk::Key::V))
        || (shift && !ctrl && key == gdk::Key::Insert)
}

/// The context menu's Paste, which reaches the page the same way the chord does and so needs the
/// same answer.
///
/// ⚠️ **The one item in this menu that is not WebKit's own**, against the rule
/// `docs/composer-security.md` Gate 14 states, and for a reason that leaves no alternative: the
/// stock item cannot be *observed*. Its action is a `WebKitContextMenuGAction`, which is neither a
/// `GSimpleAction` nor carrier of an `activate` signal, so there is no way to learn that the user
/// chose Paste and no way to read the clipboard when they did. Reusing the stock item's own label
/// would need `webkit_context_menu_item_get_title`, which arrived in WebKitGTK 2.52 and is far
/// above this client's floor. So the label is the app's own, in the app's own language, and the
/// behaviour falls straight back to the stock paste for everything that is not a picture.
///
/// A view with no handler set (the reading view, or a composer still being built) keeps the stock
/// item, so nothing here can degrade a menu that was never ours to change.
pub(super) fn paste_menu_item(view: &WebView, answer: Option<&PasteAnswer>) -> ContextMenuItem {
    let Some(answer) = answer else {
        return ContextMenuItem::from_stock_action(ContextMenuAction::Paste);
    };
    let action = gio::SimpleAction::new("composer-paste", None);
    let view = view.clone();
    let answer = answer.clone();
    action.connect_activate(move |_, _| {
        log::debug!("paste: requested from the menu");
        if !take_clipboard(&view, &answer) {
            view.execute_editing_command(webkit6::EDITING_COMMAND_PASTE);
        }
    });
    ContextMenuItem::from_gaction(&action, l10n::action_paste(), None)
}

/// Whether the clipboard held something the message can take, and it is on its way in.
///
/// The question is answered **synchronously**, because its answer decides whether the keystroke
/// belongs to us or to the page, and that cannot wait for a round trip. GDK already knows what the
/// clipboard can supply; only the reads that follow are asynchronous.
fn take_clipboard(editor: &WebView, answer: &PasteAnswer) -> bool {
    let clipboard = editor.clipboard();
    let formats = clipboard.formats();
    // The offered types, never what they hold: which formats a clipboard can supply is the shape of
    // the thing, and this is the line that says why a paste did nothing on a desktop we cannot sit
    // in front of.
    let offered = formats.mime_types();
    log::debug!(
        "paste: the clipboard offers {} type(s): {}",
        offered.len(),
        offered
            .iter()
            .map(glib::GString::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Pixels first: they are the picture itself, and Gate 13 says a pasted image goes in at the
    // caret with nothing to ask. A file reference is the other shape, and is a file.
    if let Some(mime) = picture_type(&clipboard) {
        read_picture(clipboard, mime, Rc::clone(&answer.picture));
        return true;
    }
    if formats.contains_type(gdk::FileList::static_type()) {
        read_files(clipboard, Rc::clone(&answer.files));
        return true;
    }
    log::debug!(
        "paste: nothing on the clipboard for the editor, so the keystroke stays the page's"
    );
    false
}

/// Reads the clipboard's pixels and hands them to the editor, which decides what it makes of them.
fn read_picture(clipboard: gdk::Clipboard, mime: &'static str, paste: PastePicture) {
    glib::MainContext::default().spawn_local(async move {
        let picture = read_clipboard(&clipboard, mime).await.map(|bytes| {
            log::debug!(
                "paste: read {} bytes of {mime} off the clipboard",
                bytes.len()
            );
            PastedPicture {
                bytes,
                media_type: mime.to_owned(),
            }
        });
        if picture.is_none() {
            log::debug!("paste: the clipboard's picture could not be read");
        }
        paste(picture);
    });
}

/// Reads the files the clipboard names and gives them what a drop gives them.
///
/// Asked for as a `GdkFileList` rather than parsed out of `text/uri-list` by hand, which is also
/// what the drop target asks for: GDK owns the conversion, so the portal's own transfer types (what
/// a file manager's Copy actually puts on the clipboard) arrive as files here without this module
/// knowing they exist.
fn read_files(clipboard: gdk::Clipboard, accept: AcceptFiles) {
    glib::MainContext::default().spawn_local(async move {
        let value = clipboard
            .read_value_future(gdk::FileList::static_type(), glib::Priority::DEFAULT)
            .await;
        let paths: Vec<_> = value
            .ok()
            .and_then(|value| value.get::<gdk::FileList>().ok())
            .map(|list| list.files().iter().filter_map(gio::File::path).collect())
            .unwrap_or_default();
        log::debug!("paste: the clipboard names {} file(s)", paths.len());
        accept(paths);
    });
}

/// The first format in [`PICTURE_TYPES`] the clipboard can supply, or `None` when it holds no
/// picture at all.
///
/// GDK advertises what it can convert to as well as what was put there, so a screenshot copied as
/// a texture answers `image/png` here and arrives as PNG bytes.
fn picture_type(clipboard: &gdk::Clipboard) -> Option<&'static str> {
    let formats = clipboard.formats();
    PICTURE_TYPES
        .into_iter()
        .find(|mime| formats.contain_mime_type(mime))
}

/// Reads the clipboard's picture, giving up past the core's cap.
///
/// Bounded because a clipboard, unlike a file, has no length to ask for first: without this a
/// pathological one would be pulled into memory in full only to be refused for its size. The cap
/// itself is the core's, read from it rather than restated.
async fn read_clipboard(clipboard: &gdk::Clipboard, mime: &str) -> Option<Vec<u8>> {
    let (stream, _) = clipboard
        .read_future(&[mime], glib::Priority::DEFAULT)
        .await
        .ok()?;
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        let chunk = stream
            .read_bytes_future(CHUNK_BYTES, glib::Priority::DEFAULT)
            .await
            .ok()?;
        if chunk.is_empty() {
            return Some(bytes);
        }
        bytes.extend_from_slice(&chunk);
        if bytes.len() as u64 > mailcal_bindings::MAX_INLINE_IMAGE_BYTES {
            log::debug!("paste: the picture is past the size an editor may carry");
            return None;
        }
    }
}

#[cfg(test)]
#[path = "editor_paste_tests.rs"]
pub(super) mod tests;
