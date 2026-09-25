//! Sender and event text drawn natively, with the web and mail addresses in it as links: a
//! plain-text body, an event's notes, an invitation's description.
//!
//! The core finds the addresses ([`mailcal_bindings::linked_text`]); this only draws them. A link
//! in a `GtkLabel` needs markup, so every run is escaped before it is joined: the markup is ours
//! and the text inside it never is, which is what keeps `docs/rendering-security.md` Gate 8 true
//! for a label that parses. A click opens only through the gate and launcher a link in the
//! reading view uses.

use gtk::{gio, glib, prelude::*};

/// `text` as Pango markup: each run escaped, each address wrapped in an `<a href>`.
pub(crate) fn markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for run in mailcal_bindings::linked_text(text.to_owned()) {
        let escaped = glib::markup_escape_text(&run.text);
        match run.link {
            Some(link) => {
                out.push_str("<a href=\"");
                out.push_str(&glib::markup_escape_text(&link));
                out.push_str("\">");
                out.push_str(&escaped);
                out.push_str("</a>");
            }
            None => out.push_str(&escaped),
        }
    }
    out
}

/// A wrapping, selectable label showing `text` with its addresses as links.
pub(crate) fn label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_selectable(true);
    open_through_gate(&label);
    set(&label, text);
    label
}

/// Replaces what `label` shows. The label must have been through [`open_through_gate`].
pub(crate) fn set(label: &gtk::Label, text: &str) {
    label.set_markup(&markup(text));
}

/// Routes a click on one of the label's links through the shared scheme gate to the desktop's
/// handler, and never lets GTK open it on its own.
pub(crate) fn open_through_gate(label: &gtk::Label) {
    label.connect_activate_link(|label, uri| {
        if mailcal_bindings::should_open_external_link(uri.to_owned()) {
            let window = label.root().and_downcast::<gtk::Window>();
            // `GtkUriLauncher`, as the reading view uses: it goes through the portal in a
            // sandboxed build and does not block the main thread.
            gtk::UriLauncher::new(uri).launch(window.as_ref(), gio::Cancellable::NONE, |_| ());
        }
        glib::Propagation::Stop
    });
}

#[cfg(test)]
mod tests {
    use super::markup;

    #[test]
    fn sender_text_is_escaped_and_only_addresses_become_links() {
        assert_eq!(
            markup("**bold** <b>x</b> & co: https://example.com/?a=1&b=2."),
            "**bold** &lt;b&gt;x&lt;/b&gt; &amp; co: \
             <a href=\"https://example.com/?a=1&amp;b=2\">https://example.com/?a=1&amp;b=2</a>."
        );
    }

    #[test]
    fn a_quote_in_an_address_cannot_leave_the_attribute() {
        let out = markup("https://example.com/\"onclick");
        assert!(!out.contains("\"onclick"), "{out}");
    }

    #[test]
    fn text_without_an_address_is_only_escaped() {
        assert_eq!(markup("Budget & headcount"), "Budget &amp; headcount");
        assert_eq!(markup(""), "");
    }
}
