//! The widgets the folder pane is built from: one constructor per kind of row, plus the icon a
//! role takes.
//!
//! Split from `folder_pane`, which decides which rows exist and in what order, so both stay
//! under the repo line limit. The rules are `docs/folder-pane.md`.

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{AccountRow, FolderRole, FolderRow};

use super::{
    AppInput,
    folder_names::folder_label,
    folder_pane::SidebarTarget,
    mailbox,
    notice_rules::{self, AccountMark},
    row_action,
};
use crate::{l10n, ui::icons};

/// The symbolic icon for a folder's special role; a plain folder for anything without one.
///
/// Keyed on the role the core resolves (RFC 6154 SPECIAL-USE / JMAP), never on the folder's name:
/// the name is whatever the server calls it, so a name test picks the wrong icon in six of the
/// seven shipped languages, and on any server whose folders were renamed.
///
/// Inbox and Archive are bundled (`mailcal-*`), because Adwaita, the theme the GNOME runtime
/// provides, ships neither; the rest are the desktop's own. Yaru has both, which is exactly what
/// makes a themed name for either look fine on an Ubuntu desktop and broken in the Flatpak.
pub(super) fn role_icon(role: Option<&FolderRole>) -> &'static str {
    match role {
        Some(FolderRole::Inbox) => icons::INBOX,
        Some(FolderRole::Drafts) => icons::DRAFTS,
        Some(FolderRole::Sent) => icons::SENT,
        Some(FolderRole::Archive) => icons::ARCHIVE,
        Some(FolderRole::Junk) => icons::JUNK,
        Some(FolderRole::Trash) => icons::TRASH,
        // A role we recognise but draw no distinct icon for (important, flagged), and every
        // ordinary custom folder, take the plain folder.
        Some(FolderRole::Important | FolderRole::Other) | None => icons::FOLDER,
    }
}

pub(super) fn unified_inbox_row(unread: u32, sender: &relm4::Sender<AppInput>) -> adw::ActionRow {
    let row = pane_row(
        icons::INBOX,
        l10n::folder_inbox(),
        &disclosure_slot(),
        sender,
        &SidebarTarget::AllInboxes,
    );
    add_badge(&row, unread);
    row
}

/// Draws `row` heavier than the rows beneath it: the account and All Accounts rows title their
/// groups by weight, not by indenting what they hold (`docs/folder-pane.md`, rule 32).
fn titles_group(row: &adw::ActionRow) {
    row.add_css_class(GROUP_TITLE);
}

/// The style class [`titles_group`] adds, which the pane's stylesheet draws semibold.
pub(super) const GROUP_TITLE: &str = "mailcal-group-title";

pub(super) fn unified_group_row(
    expanded: bool,
    sender: &relm4::Sender<AppInput>,
) -> adw::ActionRow {
    let row = mailbox::plain_text_row();
    row.add_css_class(ROW);
    row.set_title(l10n::sidebar_all_accounts());
    row.set_title_lines(1);
    titles_group(&row);
    row.set_activatable(true);
    let button = gtk::Button::from_icon_name(if expanded {
        icons::EXPANDED
    } else {
        icons::COLLAPSED
    });
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    let spoken = if expanded {
        l10n::a11y_collapse_account()
    } else {
        l10n::a11y_expand_account()
    };
    button.set_tooltip_text(Some(spoken));
    button.update_property(&[AccessibleProperty::Label(l10n::sidebar_all_accounts())]);
    let input = sender.clone();
    button.connect_clicked(move |_| {
        input.emit(AppInput::ActivateSidebar(SidebarTarget::UnifiedGroup));
    });
    in_disclosure_column(&button);
    row.add_prefix(&button);
    row.set_activatable_widget(Some(&button));
    row
}

/// An account: its address, a chevron that opens its tree, and **no count**; the counts belong to
/// the folders, and a roll-up here would sit directly above an identical number on the Inbox row
/// beneath it.
///
/// While a notice is about it, its mark (`docs/notices.md` rule 6): a few words on one line under
/// the address, and the first notice's glyph at the row's end, which is where an `AdwActionRow`
/// takes a widget. A screen reader hears every one of its sentences.
pub(super) fn account_row(
    account: &AccountRow,
    mark: Option<&AccountMark>,
    sender: &relm4::Sender<AppInput>,
) -> adw::ActionRow {
    let row = pane_row(
        icons::ACCOUNT,
        &account.email,
        &chevron(account, sender),
        sender,
        &SidebarTarget::Account(account.id.clone()),
    );
    titles_group(&row);
    row.add_css_class(ACCOUNT);
    // An address is as long as it is, and the pane has a floor: the row that gets truncated is
    // precisely the one the user needs to read.
    row.set_tooltip_text(Some(&account.email));
    if let Some(mark) = mark {
        row.set_subtitle(&mark.label);
        row.set_subtitle_lines(1);
        row.upcast_ref::<gtk::Widget>()
            .update_property(&[AccessibleProperty::Description(&mark.detail)]);
        let glyph = gtk::Image::from_icon_name(notice_rules::glyph(mark.severity));
        if mark.severity == mailcal_bindings::NoticeSeverity::Blocked {
            glyph.add_css_class("error");
        }
        // The words beside it say what the glyph shows.
        glyph.set_accessible_role(gtk::AccessibleRole::Presentation);
        row.add_suffix(&glyph);
    }
    row
}

/// The disclosure control, a button of its own rather than the row.
///
/// Opening a tree is not navigating (`docs/folder-pane.md` rule 2), so it must not move the
/// selection: which is exactly what activating the row does. A `GtkButton` inside the row
/// consumes its own click, so the two gestures stay separate.
fn chevron(account: &AccountRow, sender: &relm4::Sender<AppInput>) -> gtk::Button {
    let button = gtk::Button::from_icon_name(if account.expanded {
        icons::EXPANDED
    } else {
        icons::COLLAPSED
    });
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    let spoken = if account.expanded {
        l10n::a11y_collapse_account()
    } else {
        l10n::a11y_expand_account()
    };
    button.set_tooltip_text(Some(spoken));
    button.update_property(&[AccessibleProperty::Label(spoken)]);
    in_disclosure_column(&button);
    let input = sender.clone();
    let id = account.id.clone();
    let expanded = account.expanded;
    button.connect_clicked(move |_| {
        input.emit(AppInput::SetAccountExpanded {
            account: id.clone(),
            expanded: !expanded,
        });
    });
    button
}

pub(super) fn folder_row(
    account: &str,
    folder: &FolderRow,
    sender: &relm4::Sender<AppInput>,
) -> adw::ActionRow {
    // Only where there is a tree to open: a chevron beside a folder holding nothing but mail
    // promises one that is not there, so that row keeps the column empty.
    let disclosure: gtk::Widget = if folder.has_children {
        folder_chevron(account, folder, sender).upcast()
    } else {
        disclosure_slot().upcast()
    };
    let row = pane_row(
        role_icon(folder.role.as_ref()),
        &folder_label(folder.role.as_ref(), &folder.name),
        &disclosure,
        sender,
        &SidebarTarget::Folder {
            account: account.to_owned(),
            key: folder.key.clone(),
        },
    );
    // In its account's columns, and one step in per folder it is filed inside, so the tree reads
    // as a tree while the account row's weight titles it (`docs/folder-pane.md`, rule 32). A
    // margin rather than a nested list: the rows stay siblings, which is what keeps one keyboard
    // traversal over the whole pane.
    row.set_margin_start(TREE_STEP * i32::try_from(folder.depth).unwrap_or(0));
    add_badge(&row, folder.unread);
    row
}

/// One step of the destination pickers' indent, per level of folders inside folders.
pub(super) const INDENT: i32 = 18;

/// The width of the pane's disclosure column, a chevron's target.
const DISCLOSURE_WIDTH: i32 = 24;

/// The gap between the disclosure column, the icon and the name.
const GAP: i32 = 6;

/// One step of the pane's own indent: a disclosure column and its gap, so a folder inside a folder
/// has its chevron under its parent's icon and the branch hangs from the row that opened it.
const TREE_STEP: i32 = DISCLOSURE_WIDTH + GAP;

/// The pane's rows, set closer than a list row's defaults: a sidebar is scanned rather than read,
/// and the generic 50 px row and two full-size buttons before every name spent a third of the
/// pane's width and height on padding. Sizes only; every colour stays the theme's.
fn pane_css() -> String {
    format!(
        ".navigation-sidebar > row.{ROW} {{ min-height: 32px; padding: 0 4px; }}
row.{ROW} > box.header {{ margin-left: 4px; margin-right: 4px; min-height: 32px; border-spacing: {GAP}px; }}
row.{ROW} > box.header > box.title {{ margin-top: 2px; margin-bottom: 2px; }}
row.{ROW} > box.header > .prefixes {{ border-spacing: {GAP}px; margin-right: {GAP}px; }}
row.{ROW} > box.header > .prefixes:dir(rtl) {{ margin-right: 0; margin-left: {GAP}px; }}
button.{DISCLOSURE} {{ min-width: {DISCLOSURE_WIDTH}px; min-height: {DISCLOSURE_WIDTH}px; padding: 0; }}
.navigation-sidebar > row.{ACCOUNT} {{ margin-top: 10px; }}
row.{GROUP_TITLE} .title {{ font-weight: 600; }}"
    )
}

/// Every pane row's style class.
pub(super) const ROW: &str = "mailcal-pane-row";
/// A chevron's style class.
const DISCLOSURE: &str = "mailcal-pane-disclosure";
/// An account row's style class: a little space above it, so each account's tree reads as a
/// group of its own.
const ACCOUNT: &str = "mailcal-pane-account";

/// Installs the pane's stylesheet once per display.
pub(super) fn install_styles() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    if let Some(display) = gtk::gdk::Display::default() {
        INSTALLED.call_once(|| {
            let provider = gtk::CssProvider::new();
            provider.load_from_string(&pane_css());
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        });
    }
}

/// The disclosure control on a folder that holds folders: a button of its own, for the reason the
/// account row's chevron is one.
///
/// Opening a folder to see what is inside it is not opening its mail, so this must not move the
/// selection, which is exactly what activating the row does. A `GtkButton` inside the row consumes
/// its own click, so the two gestures stay separate.
fn folder_chevron(
    account: &str,
    folder: &FolderRow,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Button {
    let button = gtk::Button::from_icon_name(if folder.expanded {
        icons::EXPANDED
    } else {
        icons::COLLAPSED
    });
    button.add_css_class("flat");
    button.set_valign(gtk::Align::Center);
    let spoken = if folder.expanded {
        l10n::a11y_collapse_folder()
    } else {
        l10n::a11y_expand_folder()
    };
    button.set_tooltip_text(Some(spoken));
    button.update_property(&[AccessibleProperty::Label(spoken)]);
    in_disclosure_column(&button);
    let input = sender.clone();
    let account = account.to_owned();
    let key = folder.key.clone();
    let expanded = folder.expanded;
    button.connect_clicked(move |_| {
        input.emit(AppInput::SetFolderExpanded {
            account: account.clone(),
            key: key.clone(),
            expanded: !expanded,
        });
    });
    button
}

/// The shared skeleton: `disclosure` leading, then the icon carrying the row's action, then one
/// line under `title`, the whole row activatable. No arrow: the only chevron in this pane opens
/// or shuts a tree, and it leads the row, so what a tree holds stands under it.
pub(super) fn pane_row(
    icon: &str,
    title: &str,
    disclosure: &impl IsA<gtk::Widget>,
    sender: &relm4::Sender<AppInput>,
    target: &SidebarTarget,
) -> adw::ActionRow {
    // A folder's name and an account's address are the server's text: a bare ampersand must not
    // be read as an entity, and a markup-shaped name must render as itself.
    let row = mailbox::plain_text_row();
    row.add_css_class(ROW);
    row.set_title(title);
    row.set_title_lines(1);
    row.set_activatable(true);
    let input = sender.clone();
    let target = target.clone();
    row_action::icon_action_row(&row, icon, title, move || {
        input.emit(AppInput::ActivateSidebar(target.clone()));
    });
    // After the icon, because `AdwActionRow` puts each new prefix before the ones it holds.
    row.add_prefix(disclosure);
    row
}

thread_local! {
    /// Every chevron in the pane and every empty slot standing in for one: the toolkit gives them
    /// all the width of the widest, so an icon stands in the same column whether its row can open
    /// or not (`docs/folder-pane.md`, rule 32).
    static DISCLOSURE_COLUMN: gtk::SizeGroup = gtk::SizeGroup::new(gtk::SizeGroupMode::Horizontal);
}

fn in_disclosure_column(widget: &impl IsA<gtk::Widget>) {
    if let Some(button) = widget.dynamic_cast_ref::<gtk::Button>() {
        button.add_css_class(DISCLOSURE);
    }
    DISCLOSURE_COLUMN.with(|group| group.add_widget(widget));
}

/// The disclosure column of a row with nothing to open: empty, and as wide as a chevron.
pub(super) fn disclosure_slot() -> gtk::Box {
    let slot = gtk::Box::builder()
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build();
    in_disclosure_column(&slot);
    slot
}

/// The unread count at the trailing edge; **nothing at zero**, which also covers a provider that
/// reports no count at all (Gmail today). A badge reading `0` would claim we looked and found
/// nothing.
fn add_badge(row: &adw::ActionRow, unread: u32) {
    if unread == 0 {
        return;
    }
    let label = mailbox::badge(&unread.to_string());
    // The bare number reads as a position in a list; the spoken label says what it counts.
    let spoken = l10n::a11y_unread_count(i64::from(unread));
    label.set_tooltip_text(Some(&spoken));
    label.update_property(&[AccessibleProperty::Label(&spoken)]);
    row.add_suffix(&label);
}
