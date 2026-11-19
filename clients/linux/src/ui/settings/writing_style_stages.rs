//! What each stage of learning a style shows (`docs/ai.md` rule 11), drawn from the stage the core
//! is on. The controls that move the flow are the host's; these are the stage's own choices and
//! words.

use std::rc::Rc;

use adw::prelude::*;
use mailcal_bindings::{
    AiFailure, StyleLearnRange, StyleLearningAccount, StyleLearningDestination,
    StyleLearningIntent, StyleLearningReport, StyleLearningReview, StyleSentReach,
};

use super::writing_style_words as words;
use crate::{
    l10n,
    ui::{icons, mailbox::plain_text_row},
};

/// Where a stage's choice goes.
pub(super) type Send = Rc<dyn Fn(StyleLearningIntent)>;

/// A stage's content: a column no wider than reads well, scrolled when it runs long.
pub(super) fn column(content: &gtk::Box) -> gtk::Widget {
    content.set_margin_start(24);
    content.set_margin_end(24);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(560);
    clamp.set_child(Some(content));
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    // Expanding both ways: a carousel gives a page that does not only the width it asks for.
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);
    scroll.set_child(Some(&clamp));
    scroll.upcast()
}

pub(super) fn text(value: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(value));
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_xalign(0.0);
    label
}

pub(super) fn dim(value: &str) -> gtk::Label {
    let label = text(value);
    label.add_css_class("dim-label");
    label
}

/// A boxed list of choices of which one is picked: `picked` is told the position the person
/// picked, never the one drawn.
fn choices(
    rows: &[(String, Option<String>)],
    selected: Option<usize>,
    picked: impl Fn(usize) + 'static,
) -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    let picked = Rc::new(picked);
    let mut first: Option<gtk::CheckButton> = None;
    for (position, (title, subtitle)) in rows.iter().enumerate() {
        let check = gtk::CheckButton::new();
        match &first {
            Some(anchor) => check.set_group(Some(anchor)),
            None => first = Some(check.clone()),
        }
        check.set_active(selected == Some(position));
        let row = plain_text_row();
        row.set_title(title);
        if let Some(subtitle) = subtitle {
            row.set_subtitle(subtitle);
        }
        row.add_prefix(&check);
        row.set_activatable_widget(Some(&check));
        let picked = Rc::clone(&picked);
        check.connect_toggled(move |check| {
            if check.is_active() && selected != Some(position) {
                picked(position);
            }
        });
        list.append(&row);
    }
    list
}

pub(super) fn choose_account(
    accounts: &[StyleLearningAccount],
    selected: Option<&str>,
    send: &Send,
) -> gtk::Widget {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    let rows = accounts
        .iter()
        .map(|account| (account.email.clone(), None))
        .collect::<Vec<_>>();
    let at = accounts
        .iter()
        .position(|account| Some(account.account_id.as_str()) == selected);
    let (ids, send) = (
        accounts
            .iter()
            .map(|account| account.account_id.clone())
            .collect::<Vec<_>>(),
        Rc::clone(send),
    );
    content.append(&choices(&rows, at, move |position| {
        if let Some(account) = ids.get(position) {
            send(StyleLearningIntent::SelectAccount {
                account: account.clone(),
            });
        }
    }));
    column(&content)
}

/// The range stage: the core's presets, and for a range of the person's own its two days.
pub(super) struct Range<'a> {
    pub(super) email: &'a str,
    pub(super) presets: &'a [StyleLearnRange],
    pub(super) earliest: Option<&'a str>,
    pub(super) latest: &'a str,
    pub(super) selected: Option<&'a StyleLearnRange>,
    pub(super) reach: &'a StyleSentReach,
}

pub(super) fn choose_range(range: &Range<'_>, send: &Send) -> gtk::Widget {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.append(&dim(&format!(
        "{}\n{}",
        range.email,
        words::reach(range.reach)
    )));
    // The presets, then a range of the person's own, which is always offered within the bounds.
    let own = StyleLearnRange::Between {
        since: range.earliest.unwrap_or(range.latest).to_owned(),
        until: range.latest.to_owned(),
    };
    let offered = range
        .presets
        .iter()
        .filter(|preset| !matches!(preset, StyleLearnRange::Between { .. }))
        .cloned()
        .chain(std::iter::once(own))
        .collect::<Vec<_>>();
    let rows = offered
        .iter()
        .map(|choice| {
            let hint = matches!(choice, StyleLearnRange::UntilEndOfLastYear)
                .then(|| l10n::learn_range_until_hint().to_owned());
            (words::range(choice).to_owned(), hint)
        })
        .collect::<Vec<_>>();
    let at = range.selected.and_then(|selected| {
        offered
            .iter()
            .position(|choice| std::mem::discriminant(choice) == std::mem::discriminant(selected))
    });
    let (chosen, pick) = (range.selected.cloned(), Rc::clone(send));
    content.append(&choices(&rows, at, move |position| {
        let Some(choice) = offered.get(position) else {
            return;
        };
        // A range of the person's own opens on the one already chosen, or on the widest the core
        // allows.
        let range = match (choice, &chosen) {
            (StyleLearnRange::Between { .. }, Some(kept @ StyleLearnRange::Between { .. })) => {
                kept.clone()
            }
            (other, _) => other.clone(),
        };
        pick(StyleLearningIntent::SelectRange { range });
    }));
    if let Some(StyleLearnRange::Between { since, until }) = range.selected {
        let days = gtk::ListBox::new();
        days.add_css_class("boxed-list");
        days.set_selection_mode(gtk::SelectionMode::None);
        let (send_since, kept_until) = (Rc::clone(send), until.clone());
        days.append(&day_row(l10n::learn_range_from(), since, move |day| {
            send_since(StyleLearningIntent::SelectRange {
                range: StyleLearnRange::Between {
                    since: day,
                    until: kept_until.clone(),
                },
            });
        }));
        let (send_until, kept_since) = (Rc::clone(send), since.clone());
        days.append(&day_row(
            l10n::learn_range_until_label(),
            until,
            move |day| {
                send_until(StyleLearningIntent::SelectRange {
                    range: StyleLearnRange::Between {
                        since: kept_since.clone(),
                        until: day,
                    },
                });
            },
        ));
        content.append(&days);
    }
    column(&content)
}

/// A row naming a day, which opens a calendar to pick another. A day the core does not admit is
/// not taken, and the row stays on the one it holds.
fn day_row(title: &str, day: &str, picked: impl Fn(String) + 'static) -> adw::ActionRow {
    let row = plain_text_row();
    row.set_title(title);
    let calendar = gtk::Calendar::new();
    if let Some(date) = date_of(day) {
        calendar.set_date(&date);
    }
    let popover = gtk::Popover::new();
    popover.set_child(Some(&calendar));
    let button = gtk::MenuButton::new();
    button.set_label(&words::day(day));
    button.set_popover(Some(&popover));
    button.set_valign(gtk::Align::Center);
    button.update_property(&[gtk::accessible::Property::Label(title)]);
    let (drawn, shut) = (day.to_owned(), popover.clone());
    calendar.connect_day_selected(move |calendar| {
        let picked_day = calendar
            .date()
            .format("%Y-%m-%d")
            .map(|day| day.to_string());
        if let Ok(day) = picked_day
            && day != drawn
        {
            shut.popdown();
            picked(day);
        }
    });
    row.add_suffix(&button);
    row.set_activatable_widget(Some(&button));
    row
}

fn date_of(day: &str) -> Option<gtk::glib::DateTime> {
    let mut parts = day.splitn(3, '-').map(str::parse::<i32>);
    let (year, month, date) = (
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    );
    gtk::glib::DateTime::from_utc(year, month, date, 0, 0, 0.0).ok()
}

/// What the review read, where it goes and what it means for the account.
pub(super) struct Review<'a> {
    pub(super) email: &'a str,
    pub(super) range: &'a StyleLearnRange,
    pub(super) reach: &'a StyleSentReach,
    pub(super) state: &'a StyleLearningReview,
    pub(super) sends_to: Option<&'a StyleLearningDestination>,
    pub(super) cannot_send: Option<AiFailure>,
    pub(super) keyword_kept: bool,
}

pub(super) fn review(review: &Review<'_>) -> gtk::Widget {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.append(&dim(&format!(
        "{} · {}",
        review.email,
        words::range_chosen(review.range)
    )));
    match review.state {
        StyleLearningReview::Reading => {
            let reading = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            let spinner = adw::Spinner::new();
            reading.append(&spinner);
            reading.append(&text(l10n::learn_reading()));
            content.append(&reading);
        }
        StyleLearningReview::Ready { report } => content.append(&report_list(report)),
        StyleLearningReview::NothingUsable { report } => {
            content.append(&report_list(report));
            let nothing = text(l10n::learn_report_nothing());
            nothing.add_css_class("warning");
            content.append(&nothing);
        }
    }
    content.append(&dim(&words::reach(review.reach)));
    let sending = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let heading = text(l10n::learn_consent_title());
    heading.add_css_class("heading");
    heading.set_accessible_role(gtk::AccessibleRole::Heading);
    sending.append(&heading);
    match (review.sends_to, review.cannot_send) {
        (Some(StyleLearningDestination::Relay), _) => {
            sending.append(&text(l10n::learn_consent_relay()));
        }
        (Some(StyleLearningDestination::OwnEndpoint { host }), _) => {
            sending.append(&text(&l10n::learn_consent_own(host)));
        }
        (None, failure) => {
            let said = text(failure.map_or_else(l10n::ai_failure_nowhere_to_send, words::failure));
            said.add_css_class("error");
            sending.append(&said);
        }
    }
    if !review.keyword_kept {
        sending.append(&dim(l10n::learn_keyword_not_kept()));
    }
    content.append(&sending);
    column(&content)
}

fn report_list(report: &StyleLearningReport) -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    let line = |title: String| {
        let row = plain_text_row();
        row.set_title(&title);
        row
    };
    list.append(&line(l10n::learn_report_found(i64::from(report.found))));
    list.append(&line(l10n::learn_report_usable(i64::from(report.usable))));
    if !report.languages.is_empty() {
        let counts = report
            .languages
            .iter()
            .map(|language| {
                format!(
                    "{} {}",
                    l10n::language_name(&language.language),
                    language.usable
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        list.append(&line(l10n::learn_report_languages(&counts)));
    }
    if report.other_languages > 0 {
        list.append(&line(l10n::learn_report_undetected(i64::from(
            report.other_languages,
        ))));
    }
    if let (Some(oldest), Some(newest)) = (report.oldest, report.newest) {
        list.append(&line(l10n::learn_report_dates(
            &words::day_of(oldest),
            &words::day_of(newest),
        )));
    }
    list
}

/// A stage that says one thing large: the run under way, or what it came to.
fn status(title: &str, description: Option<&str>) -> adw::StatusPage {
    let page = adw::StatusPage::new();
    page.set_title(&gtk::glib::markup_escape_text(title));
    if let Some(description) = description {
        page.set_description(Some(&gtk::glib::markup_escape_text(description)));
    }
    page.set_vexpand(true);
    page
}

pub(super) fn learning(done: u32, total: u32) -> gtk::Widget {
    let page = status(&words::progress(done, total), None);
    page.set_paintable(Some(&adw::SpinnerPaintable::new(Some(&page))));
    if total > 0 {
        let bar = gtk::ProgressBar::new();
        bar.set_fraction(f64::from(done.min(total)) / f64::from(total));
        let clamp = adw::Clamp::new();
        clamp.set_maximum_size(320);
        clamp.set_child(Some(&bar));
        page.set_child(Some(&clamp));
    }
    page.upcast()
}

pub(super) fn failed(failure: AiFailure) -> gtk::Widget {
    let page = status(l10n::learn_failed_title(), Some(words::failure(failure)));
    page.set_icon_name(Some(icons::WARNING));
    page.upcast()
}
