//! What the command bar must do, and what it must never do (`docs/command-bar.md`).
//!
//! Called from the crate's single `gtk::init` test (see [`crate::ui::mailbox::tests`]).

use adw::prelude::*;
use mailcal_bindings::{Command, CommandBar, CommandItem, CommandSection, RowAction};

use super::CommandBarRow;
use crate::{
    l10n,
    ui::{AppInput, message_move::MoveContext},
};

fn act(action: RowAction, enabled: bool) -> CommandItem {
    CommandItem {
        command: Command::Act { action },
        enabled,
        target: None,
        confirm: action == RowAction::PermanentlyDelete,
    }
}

/// The core's bar over `selected` rows: what `command_bar` answers outside Trash, toggles
/// naming their affirmative halves.
fn bar(enabled: bool, delete: RowAction) -> CommandBar {
    CommandBar {
        sections: vec![
            CommandSection {
                items: vec![
                    act(delete, enabled),
                    act(RowAction::Archive, enabled),
                    act(RowAction::MoveToFolder, enabled),
                    act(RowAction::Flag, enabled),
                    act(RowAction::MarkRead, enabled),
                ],
            },
            CommandSection {
                items: vec![CommandItem {
                    command: Command::Sync,
                    enabled: true,
                    target: None,
                    confirm: false,
                }],
            },
        ],
        menu: vec![act(RowAction::MarkAsSpam, enabled)],
    }
}

/// The bar draws the core's items in its order, and nothing of its own: no Select all, no Clear
/// selection, no Delete permanently beside Delete (rules 3 and 6). With nothing selected every
/// action is disabled and keeps its place; Sync never is (rule 5).
pub(crate) fn the_bar_draws_the_cores_items_in_order_and_adds_none() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let row = CommandBarRow::new(&sender);
    row.render(&bar(false, RowAction::Delete), MoveContext::default());

    assert_eq!(
        row.labels(),
        [
            l10n::action_delete(),
            l10n::action_archive(),
            l10n::action_move(),
            l10n::action_flag(),
            l10n::action_mark_read(),
            l10n::action_refresh(),
        ]
    );
    for index in 0..5 {
        assert!(
            !row.button(index).is_sensitive(),
            "{} over nothing",
            row.labels()[index]
        );
    }
    let sync = row.button(5);
    assert!(
        sync.is_sensitive(),
        "Sync acts on the mailbox, not the selection"
    );
    sync.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::RefreshRequested)
    ));

    // A selection: what the core enables responds, and a pick is the core's own item.
    row.render(&bar(true, RowAction::Delete), MoveContext::default());
    let delete = row.button(0);
    assert!(delete.is_sensitive());
    assert!(!delete.has_css_class("destructive-action"));
    delete.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::BarPick(item)) if *item == act(RowAction::Delete, true)
    ));
    assert!(
        !row.button(2).is_sensitive(),
        "Move with nowhere to go is disabled, and keeps its place"
    );
    assert!(row.action_enabled("item-0"), "the menu's copy responds too");
    assert!(!row.action_enabled("item-2"));

    // In Trash, Delete permanently stands in Delete's place, and the pick carries the question.
    row.render(
        &bar(true, RowAction::PermanentlyDelete),
        MoveContext::default(),
    );
    assert_eq!(row.labels()[0], l10n::action_delete_permanently());
    assert_eq!(row.labels().len(), 6, "in Delete's place, never beside it");
    let purge = row.button(0);
    assert!(purge.has_css_class("destructive-action"));
    purge.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::BarPick(item)) if item.confirm
    ));
    assert!(
        std::ptr::eq(delete.as_ptr().cast::<u8>(), purge.as_ptr().cast::<u8>()),
        "a bar of the same shape is relabelled in place, so a focused item keeps its focus"
    );
}

/// Items leave from the end, Sync first, into ⋯ in bar order with the bar's dividers between
/// groups, and Report always stands there past them (rule 4). The ⋯ button is drawn only while
/// its menu holds something.
pub(crate) fn items_that_do_not_fit_move_from_the_end_into_the_menu() {
    let (sender, _receiver) = relm4::channel::<AppInput>();
    let row = CommandBarRow::new(&sender);
    row.render(&bar(true, RowAction::Delete), MoveContext::default());
    let overflow = row.overflow();

    overflow.sense(10_000);
    assert_eq!(overflow.shown(), 6, "a wide window stands every item");
    assert!(overflow.more_shown(), "Report always stands in the menu");
    assert_eq!(
        overflow.menu(),
        [vec![l10n::action_mark_as_spam().to_owned()]]
    );

    overflow.sense(0);
    assert_eq!(overflow.shown(), 0, "nothing fits beside ⋯");
    assert_eq!(
        overflow.menu(),
        [
            vec![
                l10n::action_delete().to_owned(),
                l10n::action_archive().to_owned(),
                l10n::action_move().to_owned(),
                l10n::action_flag().to_owned(),
                l10n::action_mark_read().to_owned(),
            ],
            vec![l10n::action_refresh().to_owned()],
            vec![l10n::action_mark_as_spam().to_owned()],
        ],
        "in bar order, grouped as on the bar, the extras last"
    );

    // Somewhere in between, measured rather than counted: as wide as the first two need.
    let two = natural(&row.button(0)) + natural(&row.button(1)) + natural(&row.button(2)) / 2;
    overflow.sense(two + natural_more());
    let shown = overflow.shown();
    assert!((1..6).contains(&shown), "{shown} items stand");
    let menu = overflow.menu();
    assert_eq!(
        menu.first()
            .and_then(|group| group.first())
            .map(String::as_str),
        Some(row.labels()[shown].as_str()),
        "the menu starts with the first item that did not fit"
    );

    // Without extras, ⋯ is drawn only while something has moved there.
    let mut plain = bar(true, RowAction::Delete);
    plain.menu.clear();
    row.render(&plain, MoveContext::default());
    overflow.sense(10_000);
    assert!(!overflow.more_shown(), "an empty menu has no button");
    overflow.sense(0);
    assert!(overflow.more_shown());
}

/// New Mail stands over the folder pane, in a slot as wide as that pane, so the actions start at
/// the list's leading edge and move with the divider (rule 2).
pub(crate) fn new_mail_leads_the_bar_as_wide_as_the_folder_pane() {
    let (sender, receiver) = relm4::channel::<AppInput>();
    let row = CommandBarRow::new(&sender);
    let pane = gtk::Paned::new(gtk::Orientation::Horizontal);
    pane.set_position(260);
    row.follow(&pane);
    assert_eq!(row.lead().size_request().0, 260);
    pane.set_position(410);
    assert_eq!(row.lead().size_request().0, 410, "it follows the divider");

    let compose = row.compose();
    assert!(compose.has_css_class("suggested-action"));
    assert_eq!(
        compose.parent().as_ref(),
        Some(row.lead().upcast_ref::<gtk::Widget>())
    );
    compose.emit_clicked();
    assert!(matches!(receiver.recv_sync(), Some(AppInput::BeginNew)));
}

fn natural(widget: &impl IsA<gtk::Widget>) -> i32 {
    widget.measure(gtk::Orientation::Horizontal, -1).1
}

fn natural_more() -> i32 {
    let more = gtk::MenuButton::new();
    more.set_icon_name(crate::ui::icons::MORE);
    natural(&more) + 8
}
