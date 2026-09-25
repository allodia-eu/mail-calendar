//! Dragging a folder onto a folder or its account, and mail onto a folder
//! (`docs/folder-pane.md`, rule 24).
//!
//! The payload is a boxed value of a type only this process registers, so it never leaves the
//! app: GDK offers other applications no format they could read it as.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use super::folder_actions::{Dragged, DropSpot, accepts_drop};

/// What one pane drag carries: the account it belongs to, and what is being moved.
#[derive(Clone, Debug, glib::Boxed)]
#[boxed_type(name = "MailcalPaneDrag")]
pub(crate) struct PaneDrag {
    pub(crate) account: String,
    pub(crate) dragged: Dragged,
}

/// Makes `widget` a drag source for `drag`.
pub(crate) fn source(widget: &impl IsA<gtk::Widget>, drag: PaneDrag) {
    let source = gtk::DragSource::new();
    source.set_actions(gtk::gdk::DragAction::MOVE);
    source.connect_prepare(move |_, _, _| {
        Some(gtk::gdk::ContentProvider::for_value(&drag.to_value()))
    });
    widget.add_controller(source);
}

/// Makes `widget`, a row of `account` described by `spot`, a drop target. A drag the row would
/// refuse is not accepted at all, so the row is not highlighted under it.
pub(crate) fn target(
    widget: &impl IsA<gtk::Widget>,
    account: &str,
    spot: DropSpot,
    dropped: impl Fn(PaneDrag) + 'static,
) {
    let target = gtk::DropTarget::new(PaneDrag::static_type(), gtk::gdk::DragAction::MOVE);
    let accepts = {
        let account = account.to_owned();
        Rc::new(move |drag: &PaneDrag| accepts_drop(&account, &spot, &drag.account, &drag.dragged))
    };
    let check = accepts.clone();
    target.connect_accept(move |_, drop| local_payload(drop).is_some_and(|drag| check(&drag)));
    target.connect_drop(move |_, value, _, _| {
        let Ok(drag) = value.get::<PaneDrag>() else {
            return false;
        };
        if !accepts(&drag) {
            return false;
        }
        dropped(drag);
        true
    });
    widget.add_controller(target);
}

/// The payload of a drag started in this process, read before it is released: what lets a row
/// refuse a drop it would not take rather than highlight and then do nothing.
fn local_payload(drop: &gtk::gdk::Drop) -> Option<PaneDrag> {
    let content = drop.drag()?.content();
    content
        .value(PaneDrag::static_type())
        .ok()?
        .get::<PaneDrag>()
        .ok()
}
