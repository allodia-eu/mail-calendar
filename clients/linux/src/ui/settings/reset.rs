//! Settings → Advanced's reset of the local database, drawn from the core's `advanced_page()`:
//! the button runs from the confirmation until the reset ends, then the outcome is said under
//! it. The window stays open throughout.

use std::{cell::RefCell, rc::Rc, sync::Arc};

use adw::prelude::*;
use mailcal_bindings::{AdvancedPage, ControlState, Intent, MailcalApp, ResetOutcome};

use super::{PageContext, confirm_window, group};
use crate::{l10n, ui::controls};

/// What the group is drawn from.
type Drawn = (ControlState, Option<ResetOutcome>);

fn drawn(page: &AdvancedPage) -> Drawn {
    (page.reset, page.reset_outcome)
}

/// The words for what the last reset came to.
pub(super) fn outcome_text(outcome: ResetOutcome) -> &'static str {
    match outcome {
        ResetOutcome::Done => l10n::settings_advanced_reset_done(),
        ResetOutcome::Failed => l10n::settings_advanced_reset_failed(),
    }
}

pub(super) fn group_following(ctx: &PageContext) -> gtk::Box {
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let page = ctx.app.advanced_page();
    let shown = Rc::new(RefCell::new(drawn(&page)));
    holder.append(&reset_group(&ctx.app, &ctx.window, *shown.borrow()));
    let app = ctx.app.clone();
    let parent = ctx.window.clone();
    let slot = holder.downgrade();
    ctx.follow.add(move || {
        let now = drawn(&app.advanced_page());
        let Some(holder) = slot.upgrade() else {
            return;
        };
        if *shown.borrow() == now {
            return;
        }
        if let Some(old) = holder.first_child() {
            holder.remove(&old);
        }
        holder.append(&reset_group(&app, &parent, now));
        *shown.borrow_mut() = now;
    });
    holder
}

fn reset_group(
    app: &Arc<MailcalApp>,
    parent: &gtk::Window,
    (control, outcome): Drawn,
) -> adw::PreferencesGroup {
    let group = group(
        l10n::action_reset_database(),
        l10n::settings_advanced_reset_description(),
    );
    let row = adw::ActionRow::builder()
        .title(l10n::action_reset_database())
        .subtitle(l10n::settings_advanced_reset_description())
        .use_markup(false)
        .build();
    let app = app.clone();
    let parent = parent.clone();
    let button = controls::button(l10n::action_reset_database(), control, true, move || {
        confirm(&parent, app.clone());
    });
    button.set_widget_name("reset-database");
    row.add_suffix(&button);
    group.add(&row);
    if let Some(outcome) = outcome {
        let said = adw::ActionRow::builder()
            .title(outcome_text(outcome))
            .use_markup(false)
            .build();
        said.set_widget_name("reset-outcome");
        group.add(&said);
    }
    group
}

fn confirm(parent: &gtk::Window, app: Arc<MailcalApp>) {
    let dialog = confirm_window(parent, l10n::reset_title(), l10n::reset_message());
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    let window = dialog.clone();
    cancel.connect_clicked(move |_| window.close());
    actions.append(&cancel);
    let reset = gtk::Button::with_label(l10n::reset_confirm());
    reset.add_css_class("destructive-action");
    let window = dialog.clone();
    reset.connect_clicked(move |_| {
        app.dispatch(Intent::ResetDatabase);
        window.close();
    });
    actions.append(&reset);
    dialog
        .child()
        .and_downcast::<gtk::Box>()
        .expect("confirmation content")
        .append(&actions);
    dialog.present();
}

#[cfg(test)]
mod tests {
    use mailcal_bindings::ResetOutcome;

    use super::outcome_text;
    use crate::l10n;

    #[test]
    fn each_outcome_has_its_own_words() {
        assert_eq!(
            outcome_text(ResetOutcome::Done),
            l10n::settings_advanced_reset_done()
        );
        assert_eq!(
            outcome_text(ResetOutcome::Failed),
            l10n::settings_advanced_reset_failed()
        );
    }
}
