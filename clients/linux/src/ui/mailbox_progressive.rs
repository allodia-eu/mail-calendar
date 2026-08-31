//! Keeps wholesale mailbox changes responsive by yielding between widget batches.

use std::{cell::RefCell, collections::HashSet, rc::Rc};

use adw::prelude::*;
use mailcal_bindings::{MailboxListSnapshot, SnapshotRow};

use super::{
    AppInput,
    mailbox::{ThreadKey, build_row},
    mailbox_display::{DisplayRow, display_row},
    mailbox_reconcile::{Row, reconcile},
};

/// Enough rows to fill the default-height list pane, including a small scroll margin.
pub(super) const INITIAL_ROWS: usize = 16;
const IDLE_BATCH_ROWS: usize = 16;

#[derive(Default)]
pub(super) struct ProgressiveRenderer {
    rendered: Rc<RefCell<Vec<DisplayRow>>>,
    pending: Rc<RefCell<Option<gtk::glib::SourceId>>>,
}

impl ProgressiveRenderer {
    pub(super) fn render(
        &mut self,
        list: &gtk::ListBox,
        snapshot: &MailboxListSnapshot,
        expanded: &HashSet<ThreadKey>,
        in_junk_folder: bool,
        zone: &str,
        sender: &relm4::Sender<AppInput>,
    ) {
        if let Some(source) = self.pending.borrow_mut().take() {
            source.remove();
        }
        let next: Vec<DisplayRow> = snapshot
            .rows
            .iter()
            .map(|row| display_row(row, zone))
            .collect();
        let wholesale = {
            let previous = self.rendered.borrow();
            is_wholesale_replacement(&previous, &next)
        };
        let initial = if wholesale { INITIAL_ROWS } else { next.len() };
        self.reconcile_to(
            list,
            &snapshot.rows,
            &next[..initial.min(next.len())],
            expanded,
            in_junk_folder,
            zone,
            sender,
        );
        if initial >= next.len() {
            return;
        }

        let list = list.downgrade();
        let rendered = Rc::clone(&self.rendered);
        let pending = Rc::clone(&self.pending);
        let rows = snapshot.rows.clone();
        let next = Rc::new(next);
        let expanded = expanded.clone();
        let zone = zone.to_owned();
        let sender = sender.clone();
        let source = gtk::glib::idle_add_local(move || {
            let Some(list) = list.upgrade() else {
                pending.borrow_mut().take();
                return gtk::glib::ControlFlow::Break;
            };
            let end = (rendered.borrow().len() + IDLE_BATCH_ROWS).min(next.len());
            {
                let previous = rendered.borrow();
                reconcile(&list, &previous, &next[..end], |index| {
                    build_row(&rows[index], &expanded, in_junk_folder, &zone, &sender)
                });
            }
            rendered.replace(next[..end].to_vec());
            if end == next.len() {
                pending.borrow_mut().take();
                gtk::glib::ControlFlow::Break
            } else {
                gtk::glib::ControlFlow::Continue
            }
        });
        self.pending.replace(Some(source));
    }

    #[allow(clippy::too_many_arguments)]
    fn reconcile_to(
        &self,
        list: &gtk::ListBox,
        rows: &[SnapshotRow],
        next: &[DisplayRow],
        expanded: &HashSet<ThreadKey>,
        in_junk_folder: bool,
        zone: &str,
        sender: &relm4::Sender<AppInput>,
    ) {
        {
            let previous = self.rendered.borrow();
            reconcile(list, &previous, next, |index| {
                build_row(&rows[index], expanded, in_junk_folder, zone, sender)
            });
        }
        self.rendered.replace(next.to_vec());
    }
}

fn is_wholesale_replacement(previous: &[DisplayRow], next: &[DisplayRow]) -> bool {
    if next.len() <= INITIAL_ROWS {
        return false;
    }
    if previous.is_empty() {
        return true;
    }
    let smaller = previous.len().min(next.len());
    let retained = next
        .iter()
        .filter(|row| previous.iter().any(|old| old.key() == row.key()))
        .count();
    retained * 2 < smaller
}

#[cfg(test)]
#[path = "mailbox_progressive_tests.rs"]
pub(super) mod tests;
