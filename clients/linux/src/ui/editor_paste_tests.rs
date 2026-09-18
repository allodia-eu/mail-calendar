//! The keystrokes the composer takes for itself, and the ones it must leave to the page.

use gtk::gdk;

use super::is_paste_chord;

const NONE: gdk::ModifierType = gdk::ModifierType::empty();
const CTRL: gdk::ModifierType = gdk::ModifierType::CONTROL_MASK;
const SHIFT: gdk::ModifierType = gdk::ModifierType::SHIFT_MASK;
const ALT: gdk::ModifierType = gdk::ModifierType::ALT_MASK;

#[test]
fn ctrl_v_is_a_paste() {
    assert!(is_paste_chord(gdk::Key::v, CTRL));
    // Caps Lock, or the chord held with Caps on: the key arrives upper-case and is the same paste.
    assert!(is_paste_chord(gdk::Key::V, CTRL));
}

#[test]
fn shift_insert_is_a_paste() {
    // The X11 convention, which GTK's own entries still honour.
    assert!(is_paste_chord(gdk::Key::Insert, SHIFT));
}

#[test]
fn plain_text_paste_is_left_to_the_page() {
    // Ctrl+Shift+V asks for the text without its formatting. A user asking for plain text is not
    // asking for the picture beside it, so this chord must reach the editor untouched.
    assert!(!is_paste_chord(gdk::Key::v, CTRL | SHIFT));
}

#[test]
fn a_bare_v_is_typing() {
    assert!(!is_paste_chord(gdk::Key::v, NONE));
    assert!(!is_paste_chord(gdk::Key::v, SHIFT));
}

#[test]
fn a_chord_carrying_alt_is_not_a_paste() {
    // Alt+Ctrl+V is a different binding on several desktops; taking it would swallow theirs.
    assert!(!is_paste_chord(gdk::Key::v, CTRL | ALT));
    assert!(!is_paste_chord(gdk::Key::Insert, SHIFT | ALT));
}

#[test]
fn an_insert_without_shift_is_not_a_paste() {
    assert!(!is_paste_chord(gdk::Key::Insert, NONE));
    assert!(!is_paste_chord(gdk::Key::Insert, CTRL));
}

/// The GTK half: a real clipboard, read the way a paste reads it. Called from the crate's single
/// `gtk::init` test (see [`crate::ui::mailbox::tests`]).
pub(in crate::ui) mod widget_tests {
    use std::rc::Rc;

    use gtk::{gdk, glib, prelude::*};
    use webkit6::{ContextMenuAction, ContextMenuItem};

    use super::super::{PasteAnswer, install, paste_menu_item, picture_type, read_clipboard};
    use crate::ui::{
        AppInput,
        webview::{DocumentKind, SecureWebView},
    };

    /// The composer's editor, built the way the composer builds it.
    ///
    /// ⚠️ **Never a bare `webkit6::WebView::new()`.** That takes WebKit's *default* network session
    /// rather than the ephemeral one every view in this app is given, and a test process that has
    /// created one aborts on the way out, long after the last assertion passed: the suite reports
    /// every test ok and then dies with SIGABRT, which reads as a broken harness rather than as
    /// anything to do with the test that did it.
    fn editor_view() -> SecureWebView {
        let (sender, _receiver) = relm4::channel::<AppInput>();
        SecureWebView::new(DocumentKind::Composer, sender)
    }

    /// Enough of a PNG for the core's sniff, which reads the magic number and nothing else.
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR";

    /// A paste answer that records nothing: these tests are about where the wiring goes, not what
    /// it does when it fires.
    fn answer() -> PasteAnswer {
        PasteAnswer {
            picture: Rc::new(|_| {}),
            files: Rc::new(|_| false),
        }
    }

    fn clipboard() -> gdk::Clipboard {
        gdk::Display::default()
            .expect("a display: run the suite through with-headless-session.sh")
            .clipboard()
    }

    /// The whole chain a Ctrl+V of a screenshot now takes on this toolkit: the clipboard says it
    /// holds a picture, the bytes come back off it, and the core turns them into the `data:` URI
    /// the editor is handed. None of it is reachable through the page here, which is why the host
    /// does it (see the module header).
    pub(in crate::ui) fn a_picture_on_the_clipboard_is_read_and_sniffed() {
        let clipboard = clipboard();
        clipboard
            .set_content(Some(&gdk::ContentProvider::for_bytes(
                "image/png",
                &glib::Bytes::from_static(PNG),
            )))
            .expect("the clipboard takes a picture");

        assert_eq!(picture_type(&clipboard), Some("image/png"));
        let bytes = glib::MainContext::default()
            .block_on(read_clipboard(&clipboard, "image/png"))
            .expect("the picture reads back off the clipboard");
        assert_eq!(bytes, PNG);
        let url = mailcal_bindings::composer_image_data_url_from_bytes(&bytes)
            .expect("the core takes the bytes it sniffed as a picture");
        assert!(url.starts_with("data:image/png;base64,"), "{url}");
    }

    /// A clipboard holding text is not a picture, so the keystroke stays the page's and the
    /// ordinary paste happens. Without this the host would swallow every Ctrl+V in the composer.
    pub(in crate::ui) fn text_on_the_clipboard_is_left_to_the_page() {
        let clipboard = clipboard();
        clipboard.set_text("just text");
        assert_eq!(picture_type(&clipboard), None);
    }

    /// **A file manager's Copy puts no pixels on the clipboard at all.** Copying a picture out of
    /// Files offers `text/uri-list` and the portal's own transfer types and nothing beginning
    /// `image/`, which is exactly what a paste that quietly did nothing looked like. GDK converts
    /// those to a `GdkFileList`, the same thing the drop target asks for, so the picture reaches
    /// the message by the road a dropped one takes.
    pub(in crate::ui) fn a_copied_file_is_seen_even_though_it_carries_no_pixels() {
        let clipboard = clipboard();
        let file = gtk::gio::File::for_path("/tmp/holiday.png");
        clipboard
            .set_content(Some(&gdk::ContentProvider::for_value(&file.to_value())))
            .expect("the clipboard takes a file");

        assert_eq!(
            picture_type(&clipboard),
            None,
            "a copied file offers no pixels, which is the whole trap"
        );
        assert!(
            clipboard
                .formats()
                .contains_type(gdk::FileList::static_type()),
            "a copied file reaches us as the file list a drop would carry"
        );
    }

    /// A format the core would refuse is not one the host offers to read either: the two lists are
    /// the same list, so a paste cannot reach the core with something it will only throw away.
    pub(in crate::ui) fn a_script_capable_picture_is_not_a_format_the_host_asks_for() {
        let clipboard = clipboard();
        clipboard
            .set_content(Some(&gdk::ContentProvider::for_bytes(
                "image/svg+xml",
                &glib::Bytes::from_static(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            )))
            .expect("the clipboard takes an SVG");

        assert_eq!(picture_type(&clipboard), None);
    }

    /// The chord is answered on the editor's **host**, in the capture phase, and this is the part
    /// that fails silently on a build that compiles: WebKitGTK adds key controllers of its own when
    /// it builds the view, and controllers on one widget run in the order they were added, so one
    /// added to the view afterwards is behind WebKit's and never sees a chord WebKit claimed. Only
    /// an ancestor's capture-phase controller is ahead of all of them.
    pub(in crate::ui) fn the_chord_is_answered_ahead_of_the_web_view() {
        let web = editor_view();
        let view = web.widget();
        let host = gtk::Box::new(gtk::Orientation::Vertical, 0);
        host.append(view);
        install(&host, view, answer());

        // The trap itself, asserted: the view already carries a key controller of WebKit's, so one
        // added there would be second in the same phase and would never see a claimed chord.
        let on_view = view.observe_controllers();
        assert!(
            (0..on_view.n_items())
                .filter_map(|index| on_view.item(index))
                .any(|item| item.downcast::<gtk::EventControllerKey>().is_ok()),
            "WebKit's own key controller is on the view, which is why ours is not"
        );

        let controllers = host.observe_controllers();
        let keys = (0..controllers.n_items())
            .filter_map(|index| controllers.item(index))
            .find_map(|item| item.downcast::<gtk::EventControllerKey>().ok())
            .expect("the composer answers the paste chord");
        assert_eq!(keys.propagation_phase(), gtk::PropagationPhase::Capture);
    }

    /// Why the context menu's Paste is ours rather than WebKit's, asserted rather than asserted in
    /// a comment: the stock item's action cannot be listened to, so there is no way to learn the
    /// user chose Paste and read the clipboard when they did. The day that stops being true, this
    /// test fails and the item can go back to being the stock one Gate 14 asks for.
    pub(in crate::ui) fn the_stock_paste_cannot_be_observed_which_is_why_ours_replaces_it() {
        let action = ContextMenuItem::from_stock_action(ContextMenuAction::Paste)
            .gaction()
            .expect("the stock Paste carries an action");
        assert!(
            action.downcast::<gtk::gio::SimpleAction>().is_err(),
            "the stock Paste's action can now be connected to: prefer the stock item again"
        );
    }

    /// The composer's own Paste is offered only where there is something to answer with, so the
    /// reading view's menu is untouched and a composer still being built keeps the stock item.
    pub(in crate::ui) fn only_the_composer_gets_its_own_paste_item() {
        let web = editor_view();

        let stock = paste_menu_item(web.widget(), None);
        assert_eq!(stock.stock_action(), ContextMenuAction::Paste);

        let ours = paste_menu_item(web.widget(), Some(&answer()));
        assert!(
            ours.gaction().is_some(),
            "the composer's Paste carries the action that reads the clipboard"
        );
    }
}
