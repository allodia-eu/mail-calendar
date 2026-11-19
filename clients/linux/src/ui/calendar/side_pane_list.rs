//! The calendar pane's calendars (`docs/calendar-pane.md`, rules 6 to 9 and 11): each account's
//! calendars under its address, in the core's order, drawn in the folder pane's rows.

use std::collections::HashMap;

use adw::prelude::*;
use gtk::accessible::{Property as AccessibleProperty, State as AccessibleState};
use mailcal_bindings::{CalendarGroup, CalendarRow};

use super::{side_pane::PaneInput, side_pane_menu};
use crate::{
    l10n,
    ui::{
        AppInput,
        folder_pane_rows::{self, GROUP_TITLE, ROW},
        icons, mailbox,
        notice_rules::AccountMark,
    },
};

/// The list of calendars, scrolling on its own under the mini month.
pub(crate) struct CalendarList {
    scroll: gtk::ScrolledWindow,
    list: gtk::ListBox,
    /// What the rows show, so an unchanged pull leaves them, and the focus on one, alone.
    rendered: Option<Vec<GroupKey>>,
    /// What each row stands for, in list order, so the focus follows its row through a rebuild.
    rows: Vec<RowIdentity>,
    sender: relm4::Sender<AppInput>,
}

/// The fields a group draws; the list is rebuilt when they change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct GroupKey {
    account: String,
    address: String,
    expanded: bool,
    mark: Option<AccountMark>,
    calendars: Vec<CalendarKey>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CalendarKey {
    id: String,
    name: String,
    hex: String,
    visible: bool,
    can_write: bool,
}

/// One row of the list: a group's heading, or a calendar.
#[derive(Clone, Debug, PartialEq, Eq)]
enum RowIdentity {
    Group(String),
    Calendar(String, String),
}

pub(super) fn keys(
    groups: &[CalendarGroup],
    marks: &HashMap<String, AccountMark>,
) -> Vec<GroupKey> {
    groups
        .iter()
        .map(|group| GroupKey {
            account: group.account.clone(),
            address: group.address.clone(),
            expanded: group.expanded,
            mark: marks.get(&group.account).cloned(),
            calendars: group
                .calendars
                .iter()
                .map(|calendar| CalendarKey {
                    id: calendar.id.clone(),
                    name: calendar.name.clone(),
                    hex: calendar.color.hex.clone(),
                    visible: calendar.visible,
                    can_write: calendar.can_write,
                })
                .collect(),
        })
        .collect()
}

impl CalendarList {
    pub(super) fn new(sender: &relm4::Sender<AppInput>) -> Self {
        folder_pane_rows::install_styles();
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("navigation-sidebar");
        list.update_property(&[AccessibleProperty::Label(l10n::calendar_pane_calendars())]);
        list.set_widget_name("calendar-list");
        let empty = gtk::Label::new(Some(l10n::calendar_manage_empty()));
        empty.add_css_class("dim-label");
        empty.set_margin_top(12);
        empty.set_wrap(true);
        list.set_placeholder(Some(&empty));
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&list));
        Self {
            scroll,
            list,
            rendered: None,
            rows: Vec::new(),
            sender: sender.clone(),
        }
    }

    pub(super) fn widget(&self) -> &gtk::ScrolledWindow {
        &self.scroll
    }

    /// Draws `groups`, each heading wearing its account's mark from `marks` (`docs/notices.md`
    /// rule 6), which is keyed by the account id a group carries.
    pub(super) fn render(
        &mut self,
        groups: &[CalendarGroup],
        marks: &HashMap<String, AccountMark>,
    ) {
        let drawn = keys(groups, marks);
        if self.rendered.as_ref() == Some(&drawn) {
            return;
        }
        let focused = self.focused();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.rows.clear();
        for group in groups {
            let mark = marks.get(&group.account);
            self.list.append(&group_row(group, mark, &self.sender));
            self.rows.push(RowIdentity::Group(group.account.clone()));
            if !group.expanded {
                continue;
            }
            for calendar in &group.calendars {
                self.list.append(&calendar_row(calendar, &self.sender));
                self.rows.push(RowIdentity::Calendar(
                    calendar.account.clone(),
                    calendar.id.clone(),
                ));
            }
        }
        if let Some(focused) = focused
            && let Some(index) = self.rows.iter().position(|row| *row == focused)
            && let Some(row) = self.list.row_at_index(i32::try_from(index).unwrap_or(-1))
        {
            row.grab_focus();
        }
        self.rendered = Some(drawn);
    }

    /// The row the keyboard focus is on, if it is in this list.
    fn focused(&self) -> Option<RowIdentity> {
        let row = self
            .list
            .focus_child()?
            .downcast::<gtk::ListBoxRow>()
            .ok()?;
        self.rows.get(usize::try_from(row.index()).ok()?).cloned()
    }

    #[cfg(test)]
    pub(super) fn list(&self) -> &gtk::ListBox {
        &self.list
    }
}

/// An account's heading: its address, heavier, after the chevron that opens or shuts it, and the
/// account's notice mark as the folder pane's account row wears it. Opening a group is the row's
/// only action, so activating the row is pressing the chevron.
fn group_row(
    group: &CalendarGroup,
    mark: Option<&AccountMark>,
    sender: &relm4::Sender<AppInput>,
) -> adw::ActionRow {
    let row = mailbox::plain_text_row();
    row.add_css_class(ROW);
    row.add_css_class(GROUP_TITLE);
    row.set_title(&group.address);
    row.set_title_lines(1);
    row.set_tooltip_text(Some(&group.address));
    let chevron = gtk::Button::from_icon_name(if group.expanded {
        icons::EXPANDED
    } else {
        icons::COLLAPSED
    });
    chevron.add_css_class("flat");
    chevron.set_valign(gtk::Align::Center);
    let spoken = if group.expanded {
        l10n::a11y_collapse_account()
    } else {
        l10n::a11y_expand_account()
    };
    chevron.set_tooltip_text(Some(spoken));
    chevron.update_property(&[AccessibleProperty::Label(spoken)]);
    chevron.update_state(&[AccessibleState::Expanded(Some(group.expanded))]);
    let (input, account, expanded) = (sender.clone(), group.account.clone(), group.expanded);
    chevron.connect_clicked(move |_| {
        input.emit(AppInput::CalendarPane(PaneInput::Expand {
            account: account.clone(),
            expanded: !expanded,
        }));
    });
    let icon = gtk::Image::from_icon_name(icons::ACCOUNT);
    icon.set_accessible_role(gtk::AccessibleRole::Presentation);
    // `AdwActionRow` puts each new prefix before the ones it holds.
    row.add_prefix(&icon);
    row.add_prefix(&chevron);
    row.set_activatable_widget(Some(&chevron));
    if let Some(mark) = mark {
        folder_pane_rows::wear_mark(&row, mark);
    }
    row
}

/// A calendar: its colour check, its name and, when it cannot be written, a lock. The whole row
/// is the checkbox, and pressing it shows or hides the calendar; the row is drawn again from the
/// pull that follows, never from a state of its own.
fn calendar_row(calendar: &CalendarRow, sender: &relm4::Sender<AppInput>) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .accessible_role(gtk::AccessibleRole::Checkbox)
        .use_markup(false)
        .title(&calendar.name)
        .title_lines(1)
        .activatable(true)
        .build();
    row.add_css_class(ROW);
    let spoken = row.upcast_ref::<gtk::Widget>();
    spoken.update_property(&[AccessibleProperty::Label(
        &l10n::a11y_calendar_pane_calendar(&calendar.name, &calendar.account_address),
    )]);
    spoken.update_state(&[AccessibleState::Checked(if calendar.visible {
        gtk::AccessibleTristate::True
    } else {
        gtk::AccessibleTristate::False
    })]);
    row.add_prefix(&side_pane_menu::check(
        &calendar.color.hex,
        calendar.visible,
    ));
    row.add_prefix(&folder_pane_rows::disclosure_slot());
    if !calendar.can_write {
        let lock = gtk::Image::from_icon_name(icons::READ_ONLY);
        lock.add_css_class("dim-label");
        lock.set_tooltip_text(Some(l10n::calendar_pane_read_only()));
        lock.update_property(&[AccessibleProperty::Label(l10n::calendar_pane_read_only())]);
        row.add_suffix(&lock);
        spoken.update_property(&[AccessibleProperty::Description(
            l10n::calendar_pane_read_only(),
        )]);
    }
    let (input, account, id, visible) = (
        sender.clone(),
        calendar.account.clone(),
        calendar.id.clone(),
        calendar.visible,
    );
    row.connect_activated(move |_| {
        input.emit(AppInput::CalendarPane(PaneInput::Show {
            account: account.clone(),
            calendar: id.clone(),
            visible: !visible,
        }));
    });
    side_pane_menu::attach(&row, calendar, sender);
    row
}
