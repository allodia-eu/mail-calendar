//! The composer's signature half: what a draft opens with, what happens when the sender changes,
//! and the per-message override.
//!
//! The three pure rules; which slot a kind seeds from, the resolution precedence, and the seed
//! payload; are free functions so the crate's test suite can pin them without a window, the same
//! split Apple, Android and Windows make. All three are silent when wrong: a mis-mapped slot sends
//! the reply signature on a new message and nobody notices until a recipient mentions it; a
//! resolution that ignores an explicit choice undoes a deliberate act on send; and a seed payload
//! with the wrong key names reaches `setComposerSignature` as an object with no `body_html`, which
//! the editor reads as "remove the signature", so the message simply goes out without one.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use adw::prelude::*;
use gtk::{gio, glib};
use mailcal_bindings::{MailcalApp, SignatureBody, SignatureSlotKind};
use serde_json::json;
use webkit6::prelude::WebViewExt;

use super::composer_model::ComposeKind;
use crate::l10n;

/// What signature this one message carries, when the user has said so explicitly.
///
/// **The absence of a choice**; the state a composer opens in; means FOLLOW THE ACCOUNT: it
/// re-resolves whenever the From picker changes, which is what a user who never touched the picker
/// expects, their work signature when sending from work. Once they do pick, the choice sticks even
/// across a From change: they chose it *for this message*, and silently replacing it would undo a
/// deliberate act. (Outlook re-swaps regardless; it is its most complained-about composer
/// behaviour.)
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SignatureChoice {
    /// The picker's **None**: an explicit "no signature on this message", which is not the same
    /// as having made no choice.
    NoSignature,
    /// This signature, by id.
    Named(String),
}

/// Which of the account's two slots a composer opened for `kind` seeds from.
///
/// A reply, a reply-all and a forward share one slot (Outlook's grouping): all three continue an
/// existing message, and splitting them produces a setting nobody sets.
pub(super) fn signature_slot(kind: ComposeKind) -> SignatureSlotKind {
    match kind {
        ComposeKind::New => SignatureSlotKind::NewMessage,
        ComposeKind::Reply | ComposeKind::ReplyAll | ComposeKind::Forward => {
            SignatureSlotKind::ReplyForward
        }
    }
}

/// The signature on this message right now: the user's explicit choice if they made one, else
/// whatever `account` assigns for this `kind`.
///
/// The two lookups are passed in rather than read from the core here, so this stays testable; and
/// they run on every call rather than being cached, because an assignment can change under an open
/// composer.
pub(super) fn resolve(
    choice: Option<&SignatureChoice>,
    account: Option<&str>,
    kind: ComposeKind,
    for_account: impl Fn(&str, SignatureSlotKind) -> Option<SignatureBody>,
    by_id: impl Fn(&str) -> Option<SignatureBody>,
) -> Option<SignatureBody> {
    match choice {
        None => account.and_then(|account| for_account(account, signature_slot(kind))),
        Some(SignatureChoice::NoSignature) => None,
        Some(SignatureChoice::Named(id)) => by_id(id),
    }
}

/// The `setComposerSignature` argument: the shape the Rust composer's `Block::Signature`
/// round-trips, so what the editor hands back on submit is what the core already understands.
/// `null` for no signature, which the editor seam reads as "remove the region".
pub(super) fn signature_seed(body: Option<&SignatureBody>) -> String {
    body.map_or_else(
        || "null".to_owned(),
        |body| json!({ "body_html": body.body_html, "body_plain": body.body_plain }).to_string(),
    )
}

/// The composer's live signature control: the action-bar menu, and the swap that follows the
/// sender.
pub(super) struct SignatureControl {
    app: Arc<MailcalApp>,
    kind: ComposeKind,
    /// The From picker's rows, in its own order, so a selected index names an account id.
    accounts: Vec<String>,
    from: gtk::DropDown,
    editor: webkit6::WebView,
    /// The library's ids and names, read once when the composer opens; it feeds the menu, which
    /// is built once. The **bodies** are never cached: those are resolved on every seed and every
    /// swap.
    library: Vec<(String, String)>,
    choice: RefCell<Option<SignatureChoice>>,
    /// The menu's radio state: the id currently in force, or `""` for none. Derived from the
    /// resolution on every apply, never stored on the items.
    action: gio::SimpleAction,
    button: gtk::MenuButton,
    /// Whether the bundle has parsed. A `window.setComposerSignature` sent before that lands on an
    /// undefined function and `evaluate_javascript` reports nothing, so a swap that raced the load
    /// would be silently dropped.
    ready: Cell<bool>,
}

impl SignatureControl {
    /// Builds the control, or `None` when there is nothing to offer.
    ///
    /// An empty library hides the picker entirely, as on every other platform: a menu whose only
    /// entry is "None" tells the user nothing, and with no signature written no account can have
    /// one assigned, so there is also nothing to seed or swap.
    pub(super) fn new(
        app: Option<&Arc<MailcalApp>>,
        kind: ComposeKind,
        accounts: &[(String, String)],
        from: &gtk::DropDown,
        editor: &webkit6::WebView,
    ) -> Option<Rc<Self>> {
        let app = app?;
        let library = app
            .signatures()
            .signatures
            .into_iter()
            .map(|row| (row.id, row.name))
            .collect::<Vec<_>>();
        if library.is_empty() {
            return None;
        }
        let action = gio::SimpleAction::new_stateful(
            "choose",
            Some(glib::VariantTy::STRING),
            &String::new().to_variant(),
        );
        let control = Rc::new(Self {
            app: Arc::clone(app),
            kind,
            accounts: accounts.iter().map(|(id, _)| id.clone()).collect(),
            from: from.clone(),
            editor: editor.clone(),
            library,
            choice: RefCell::new(None),
            action,
            button: gtk::MenuButton::new(),
            ready: Cell::new(false),
        });
        control.build_button();
        control.connect_sender_changes();
        Some(control)
    }

    pub(super) fn widget(&self) -> &gtk::MenuButton {
        &self.button
    }

    /// The seed for the page-finished batch, and the moment the control starts accepting swaps.
    ///
    /// It rides that batch for the same reason every other open-time hook does: sent any earlier
    /// it lands on an undefined `window.setComposerSignature`, and the composer opens with no
    /// signature at all, with nothing logged.
    pub(super) fn seed_script(&self) -> String {
        self.ready.set(true);
        let body = self.current();
        self.mark(body.as_ref());
        format!(
            "window.setComposerSignature({});",
            signature_seed(body.as_ref())
        )
    }

    /// Pushes the resolved signature into the editor and re-marks the menu.
    ///
    /// Safe to call at any time: the seam replaces only **this message's** signature region; the
    /// one that is a direct child of the editor, never a quoted original's; so the user's typed
    /// text, their trimming of the quote and the caret all stay where they are.
    fn apply(&self) {
        if !self.ready.get() {
            return;
        }
        let body = self.current();
        let script = format!(
            "window.setComposerSignature({});",
            signature_seed(body.as_ref())
        );
        self.editor
            .evaluate_javascript(&script, None, None, None::<&gio::Cancellable>, |_| {});
        self.mark(body.as_ref());
    }

    /// The signature this message should carry right now, per the shared precedence rule.
    fn current(&self) -> Option<SignatureBody> {
        let account = self.selected_account();
        resolve(
            self.choice.borrow().as_ref(),
            account.as_deref(),
            self.kind,
            |account, slot| self.app.resolve_signature(account.to_owned(), slot),
            |id| self.app.signature_body(id.to_owned()),
        )
    }

    fn selected_account(&self) -> Option<String> {
        usize::try_from(self.from.selected())
            .ok()
            .and_then(|index| self.accounts.get(index))
            .cloned()
    }

    /// Moves the menu's radio mark onto whatever is in force. Setting the state does not activate
    /// the action, so re-marking cannot re-enter the handler below.
    fn mark(&self, body: Option<&SignatureBody>) {
        let current = body.map_or("", |body| body.id.as_str());
        self.action.set_state(&current.to_variant());
    }

    fn build_button(self: &Rc<Self>) {
        let menu = signature_menu(&self.library);
        let group = gio::SimpleActionGroup::new();
        group.add_action(&self.action);
        self.button.set_label(l10n::compose_signature_label());
        self.button.set_menu_model(Some(&menu));
        self.button.insert_action_group("signature", Some(&group));

        let control = Rc::downgrade(self);
        self.action.connect_activate(move |_, target| {
            let Some(control) = control.upgrade() else {
                return;
            };
            // Picking from the menu is an explicit choice, **including** "None": which is why it
            // becomes `NoSignature` rather than clearing the choice. Clearing it would mean
            // "follow the account", and the next From change would put a signature back on a
            // message the user had just taken it off.
            let chosen = target.and_then(glib::Variant::str).unwrap_or_default();
            let choice = if chosen.is_empty() {
                SignatureChoice::NoSignature
            } else {
                SignatureChoice::Named(chosen.to_owned())
            };
            control.choice.replace(Some(choice));
            control.apply();
        });
    }

    /// Re-resolves when the From account changes, so a work signature never goes out under a
    /// personal address; the failure the setting exists to prevent, which is why it is automatic
    /// rather than a reminder. An explicit per-message choice resolves to itself and so survives
    /// untouched.
    fn connect_sender_changes(self: &Rc<Self>) {
        // Weak, so the control (which holds the picker) and the picker's handler do not keep each
        // other alive past the draft they belong to.
        let control = Rc::downgrade(self);
        self.from.connect_selected_notify(move |_| {
            if let Some(control) = control.upgrade() {
                control.apply();
            }
        });
    }
}

/// The picker's menu: the library plus **None**.
///
/// Every entry targets one stateful action, which is what makes GTK draw the current choice with
/// its own radio mark and announce it as selected; where a check glyph painted onto a plain item
/// would be decoration nobody using a screen reader could perceive.
fn signature_menu(library: &[(String, String)]) -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append_item(&menu_item(l10n::settings_signatures_none(), ""));
    for (id, name) in library {
        menu.append_item(&menu_item(name, id));
    }
    menu
}

/// One menu entry, targeting the shared radio action.
///
/// The target rides as a value rather than in a detailed action string: a signature's id is opaque
/// CSPRNG output, and `"signature.choose::<id>"` would have to survive that parser.
fn menu_item(label: &str, id: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), None);
    item.set_action_and_target_value(Some("signature.choose"), Some(&id.to_variant()));
    item
}

#[cfg(test)]
mod tests {
    use gtk::{gio, glib, prelude::*};
    use mailcal_bindings::{SignatureBody, SignatureSlotKind};

    use super::{
        ComposeKind, SignatureChoice, resolve, signature_menu, signature_seed, signature_slot,
    };

    fn body(id: &str) -> SignatureBody {
        SignatureBody {
            id: id.to_owned(),
            body_html: format!("<p>{id}</p>"),
            body_plain: id.to_owned(),
        }
    }

    #[test]
    fn a_reply_a_reply_all_and_a_forward_share_one_slot() {
        assert!(matches!(
            signature_slot(ComposeKind::New),
            SignatureSlotKind::NewMessage
        ));
        for kind in [
            ComposeKind::Reply,
            ComposeKind::ReplyAll,
            ComposeKind::Forward,
        ] {
            assert!(
                matches!(signature_slot(kind), SignatureSlotKind::ReplyForward),
                "{kind:?} continues an existing message, so it seeds from the reply slot"
            );
        }
    }

    #[test]
    fn no_choice_follows_the_account_and_its_slot() {
        let resolved = resolve(
            None,
            Some("work"),
            ComposeKind::Reply,
            |account, slot| {
                assert_eq!(account, "work");
                assert!(matches!(slot, SignatureSlotKind::ReplyForward));
                Some(body("reply-signature"))
            },
            |_| panic!("no explicit choice means no lookup by id"),
        );

        assert_eq!(
            resolved.expect("the account assigns one").id,
            "reply-signature"
        );
    }

    #[test]
    fn an_explicit_choice_wins_over_the_account_in_both_directions() {
        let account = |_: &str, _: SignatureSlotKind| Some(body("account-signature"));

        // "None" is a choice, not the absence of one: it must not fall back to the account.
        assert!(
            resolve(
                Some(&SignatureChoice::NoSignature),
                Some("work"),
                ComposeKind::New,
                account,
                |_| panic!("None resolves to nothing without a lookup"),
            )
            .is_none()
        );
        let named = resolve(
            Some(&SignatureChoice::Named("picked".to_owned())),
            Some("work"),
            ComposeKind::New,
            account,
            |id| Some(body(id)),
        );
        assert_eq!(named.expect("the picked signature").id, "picked");
    }

    #[test]
    fn a_composer_with_no_sender_yet_resolves_to_nothing() {
        assert!(
            resolve(
                None,
                None,
                ComposeKind::New,
                |_, _| panic!("there is no account to ask about"),
                |_| panic!("and no id to look up"),
            )
            .is_none()
        );
    }

    #[test]
    fn the_seed_carries_the_two_keys_the_editor_and_the_core_share() {
        let seed = signature_seed(Some(&body("work")));
        let parsed: serde_json::Value = serde_json::from_str(&seed).expect("valid JSON");

        // The Rust field names verbatim: the editor reads `body_html`/`body_plain` and emits the
        // same two back inside the Signature block. A renamed key is read as "no signature".
        assert_eq!(parsed["body_html"], "<p>work</p>");
        assert_eq!(parsed["body_plain"], "work");
        // `null`, not `{}`: the seam reads it as "remove the region".
        assert_eq!(signature_seed(None), "null");
    }

    #[test]
    fn every_menu_entry_is_one_choice_from_a_set_none_included() {
        let library = [
            ("id-work".to_owned(), "Work".to_owned()),
            ("id-home".to_owned(), "Home & away".to_owned()),
        ];

        let menu = signature_menu(&library);

        assert_eq!(menu.n_items(), 3, "the library, plus None");
        let entries = (0..menu.n_items())
            .map(|index| {
                let label = menu
                    .item_attribute_value(index, gio::MENU_ATTRIBUTE_LABEL, None)
                    .and_then(|value| value.get::<String>())
                    .expect("every entry is labelled");
                let action = menu
                    .item_attribute_value(index, gio::MENU_ATTRIBUTE_ACTION, None)
                    .and_then(|value| value.get::<String>())
                    .expect("every entry targets the radio action");
                let target = menu
                    .item_attribute_value(index, gio::MENU_ATTRIBUTE_TARGET, None)
                    .and_then(|value| value.get::<String>())
                    .expect("every entry carries the id it selects");
                (label, action, target)
            })
            .collect::<Vec<_>>();

        // One action for all three, so GTK draws exactly one mark and moves it: and so "None" is
        // a choice the user can make rather than the absence of one.
        assert!(
            entries
                .iter()
                .all(|(_, action, _)| action == "signature.choose")
        );
        assert_eq!(entries[0].0, "None");
        assert_eq!(entries[0].2, "", "None selects no signature");
        assert_eq!(
            entries[1],
            (
                "Work".to_owned(),
                "signature.choose".to_owned(),
                "id-work".to_owned()
            )
        );
        // A menu label is plain text, so a name with an ampersand reaches the menu intact.
        assert_eq!(entries[2].0, "Home & away");
        assert_eq!(entries[2].2, "id-home");
    }

    #[test]
    fn the_action_carries_the_id_the_handler_reads_back() {
        // The target is a value rather than part of a detailed action name: a signature id is
        // opaque CSPRNG output, and `"signature.choose::<id>"` would have to survive that parser.
        let item = super::menu_item("Work", "id-work");
        let target = item
            .attribute_value(gio::MENU_ATTRIBUTE_TARGET, Some(glib::VariantTy::STRING))
            .expect("a string target");

        assert_eq!(target.str(), Some("id-work"));
    }
}
