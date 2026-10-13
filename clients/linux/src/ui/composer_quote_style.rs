//! The composer's per-message quote style picker: shown only for a message carrying a quoted
//! original, and only when the user has opted into choosing per message in Settings, so an
//! ordinary reply uses the app default and the action bar stays uncluttered.

use std::{cell::Cell, rc::Rc, sync::Arc};

use adw::prelude::*;
use gtk::{accessible::Property as AccessibleProperty, gio};
use mailcal_bindings::{MailcalApp, QuoteStyleKind};
use webkit6::prelude::WebViewExt;

use super::composer_quote::{quote_style_of, style_token};
use crate::l10n;

/// The picker's rows, in its own order, so a selected index names a style.
static STYLES: [QuoteStyleKind; 2] = [QuoteStyleKind::Indented, QuoteStyleKind::LineAndHeader];

/// The dropdown and the flag that keeps a selection made here, rather than by the user, from
/// being sent back into the editor.
#[derive(Clone)]
pub(super) struct QuoteStylePicker {
    dropdown: gtk::DropDown,
    reflecting: Rc<Cell<bool>>,
}

impl QuoteStylePicker {
    /// The picker for a composer, or `None` when it has nothing to offer: no quoted original, or
    /// the per-message choice is off.
    pub(super) fn for_composer(
        has_quote: bool,
        app: Option<&Arc<MailcalApp>>,
        editor: &webkit6::WebView,
    ) -> Option<Self> {
        let settings = app.filter(|_| has_quote)?.quote_settings();
        settings
            .per_message
            .then(|| Self::new(&settings.style, editor))
    }

    fn new(style: &QuoteStyleKind, editor: &webkit6::WebView) -> Self {
        let dropdown = gtk::DropDown::from_strings(&[
            l10n::quote_style_indented(),
            l10n::quote_style_line_header(),
        ]);
        dropdown.set_selected(index_of(style));
        dropdown.set_tooltip_text(Some(l10n::quote_style_label()));
        dropdown.update_property(&[AccessibleProperty::Label(l10n::quote_style_label())]);
        let reflecting = Rc::new(Cell::new(false));
        // Weak: the editor's own load handler holds this picker, so a strong reference back would
        // keep both alive past the draft they belong to.
        let editor = editor.downgrade();
        let quiet = Rc::clone(&reflecting);
        dropdown.connect_selected_notify(move |dropdown| {
            if quiet.get() {
                return;
            }
            let (Some(editor), Some(style)) = (editor.upgrade(), style_at(dropdown.selected()))
            else {
                return;
            };
            let script = format!(
                "window.setComposerQuoteStyle({});",
                serde_json::json!(style_token(style))
            );
            editor.evaluate_javascript(&script, None, None, None::<&gio::Cancellable>, |_| {});
        });
        Self {
            dropdown,
            reflecting,
        }
    }

    pub(super) fn widget(&self) -> &gtk::DropDown {
        &self.dropdown
    }

    /// Opens the picker on the style the seeded document's quote is in: the app default for a
    /// fresh quote, and the style it was saved in for a reopened one. Nothing is sent to the
    /// editor, which already shows it.
    pub(super) fn reflect(&self, document_json: &str) {
        if let Some(style) = quote_style_of(document_json) {
            self.reflecting.set(true);
            self.dropdown.set_selected(index_of(&style));
            self.reflecting.set(false);
        }
    }
}

fn index_of(style: &QuoteStyleKind) -> u32 {
    match style {
        QuoteStyleKind::Indented => 0,
        QuoteStyleKind::LineAndHeader => 1,
    }
}

fn style_at(index: u32) -> Option<&'static QuoteStyleKind> {
    usize::try_from(index)
        .ok()
        .and_then(|index| STYLES.get(index))
}
