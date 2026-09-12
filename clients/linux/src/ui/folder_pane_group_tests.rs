//! The All Accounts group's GTK contract.

use adw::prelude::*;

use super::{pane, rows, shown, two_accounts};
use crate::ui::{AppInput, folder_pane::SidebarTarget};

/// All Accounts is a disclosure only. Its Inbox child is the unified destination and carries the
/// unread count, so shutting the group hides that child without moving any account tree.
pub(crate) fn the_unified_scope_is_an_expandable_group_with_an_inbox_child() {
    let snapshot = two_accounts();
    let (list, receiver) = pane(&snapshot);
    let text = shown(&list);
    assert!(text.iter().any(|entry| entry == "All Accounts"), "{text:?}");
    assert_eq!(
        text.iter().filter(|entry| *entry == "Inbox").count(),
        3,
        "the unified Inbox stands above both account inboxes: {text:?}"
    );

    let pane_rows = rows(&list);
    let group_action = pane_rows[0]
        .downcast_ref::<adw::ActionRow>()
        .expect("the group is an ActionRow")
        .activatable_widget()
        .and_downcast::<gtk::Button>()
        .expect("the whole group row has a native disclosure action");
    group_action.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::ActivateSidebar(SidebarTarget::UnifiedGroup))
    ));

    let inbox_action = pane_rows[1]
        .downcast_ref::<adw::ActionRow>()
        .expect("the unified Inbox is an ActionRow")
        .activatable_widget()
        .and_downcast::<gtk::Button>()
        .expect("the Inbox has a native navigation action");
    inbox_action.emit_clicked();
    assert!(matches!(
        receiver.recv_sync(),
        Some(AppInput::ActivateSidebar(SidebarTarget::AllInboxes))
    ));

    let mut collapsed = snapshot;
    collapsed.unified_expanded = false;
    let (list, _receiver) = pane(&collapsed);
    let text = shown(&list);
    assert_eq!(text.iter().filter(|entry| *entry == "Inbox").count(), 2);
    assert!(text.iter().any(|entry| entry == "All Accounts"));
}
