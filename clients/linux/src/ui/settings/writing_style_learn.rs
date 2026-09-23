//! The sheet that learns a writing style: which account, which sent mail, what that holds on this
//! device, the consent, and the run itself (`docs/ai.md`, "Learning").
//!
//! A detail inside the Settings window, as the signature editor is, for its reason: a second
//! toplevel races GTK's accessibility bridge (`docs/signatures.md`). The range and the report sit
//! on one sheet, so changing the range reads the device again at once and there is no step to
//! confirm; pressing Learn under the consent is the consent. Where the sheet stands is
//! `crate::ui::writing_style_flow`; this draws it and runs the two blocking calls off the main
//! thread.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::Arc,
};

use adw::prelude::*;
use gtk::glib;
use mailcal_bindings::{
    AiRoute, CorpusReport, LearningProgress, WritingStyleFailure, WritingStyleSnapshot,
};

use super::{PageContext, group_titled, page_box, signatures::named_row, writing_style::paragraph};
use crate::{
    l10n,
    ui::{
        blocking::off_main_thread,
        timestamps,
        writing_style::failure_text,
        writing_style_flow::{
            LearnFlow, LearnStage, Range, ReportRequest, consent_text, progress_text, report_lines,
        },
    },
};

const NAME: &str = "writing-style-learn";

/// The open sheet. The Writing style page owns it; every handler here reaches it weakly and
/// checks it has not been left, so an answer arriving afterwards lands nowhere.
pub(super) struct Sheet {
    ctx: PageContext,
    flow: RefCell<LearnFlow>,
    route: Option<AiRoute>,
    own_endpoint: Option<String>,
    zone: String,
    accounts: Vec<(String, String)>,
    range: RangeChoice,
    /// What the current stage draws, replaced whenever the stage changes.
    body: gtk::Box,
    /// A running learn's status, updated in place as progress arrives so Stop keeps its focus.
    running: RefCell<Option<Running>>,
    left: Cell<bool>,
    on_learned: Box<dyn Fn(String)>,
}

/// Opens the sheet over the Writing style page. `on_learned` receives the new style's id once the
/// sheet has gone.
pub(super) fn open(
    ctx: &PageContext,
    snapshot: &WritingStyleSnapshot,
    on_learned: impl Fn(String) + 'static,
) -> Rc<Sheet> {
    if let Some(previous) = ctx.navigation.child_by_name(NAME) {
        ctx.navigation.remove(&previous);
    }
    let page = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        l10n::writing_style_learn(),
        "",
    )));
    let cancel = gtk::Button::with_label(l10n::action_cancel());
    header.pack_start(&cancel);
    page.add_top_bar(&header);

    let content = page_box(l10n::writing_style_learn());
    let range = RangeChoice::new();
    content.append(&range.group);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.append(&body);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&content));
    page.set_content(Some(&scroll));
    ctx.navigation.add_named(&page, Some(NAME));
    ctx.navigation.set_visible_child_name(NAME);

    let accounts = snapshot
        .accounts
        .iter()
        .map(|account| (account.account_id.clone(), account.email.clone()))
        .collect::<Vec<_>>();
    let ids = accounts
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    let (flow, first) = LearnFlow::new(&ids);
    let sheet = Rc::new(Sheet {
        ctx: ctx.clone(),
        flow: RefCell::new(flow),
        route: snapshot.route,
        own_endpoint: ctx.app.own_ai_endpoint().map(|endpoint| endpoint.base_url),
        zone: ctx.app.timezone_settings().active,
        accounts,
        range,
        body,
        running: RefCell::new(None),
        left: Cell::new(false),
        on_learned: Box::new(on_learned),
    });
    let leaving = Rc::downgrade(&sheet);
    cancel.connect_clicked(move |_| {
        if let Some(sheet) = leaving.upgrade() {
            sheet.leave();
        }
    });
    sheet.connect_range();
    sheet.render();
    if let Some(request) = first {
        sheet.read(request);
    }
    sheet
}

/// The sheet, while it is still the one on screen.
fn live(sheet: &Weak<Sheet>) -> Option<Rc<Sheet>> {
    sheet.upgrade().filter(|sheet| !sheet.left.get())
}

/// The two range choices, and the day the second one ends on.
struct RangeChoice {
    group: adw::PreferencesGroup,
    everything: gtk::CheckButton,
    up_to: gtk::CheckButton,
    day_row: adw::ActionRow,
    day: gtk::MenuButton,
    calendar: gtk::Calendar,
}

impl RangeChoice {
    fn new() -> Self {
        let group = group_titled(l10n::learn_range_title());
        let everything = gtk::CheckButton::new();
        everything.set_active(true);
        let up_to = gtk::CheckButton::new();
        up_to.set_group(Some(&everything));
        group.add(&choice_row(&everything, l10n::learn_range_all(), None));
        group.add(&choice_row(
            &up_to,
            l10n::learn_range_until(),
            Some(l10n::learn_range_until_hint()),
        ));
        let calendar = gtk::Calendar::new();
        let popover = gtk::Popover::new();
        popover.set_child(Some(&calendar));
        let day = gtk::MenuButton::new();
        day.set_popover(Some(&popover));
        day.set_valign(gtk::Align::Center);
        let day_row = named_row(l10n::learn_range_until_label());
        day_row.add_suffix(&day);
        day_row.set_activatable_widget(Some(&day));
        day_row.set_visible(false);
        group.add(&day_row);
        Self {
            group,
            everything,
            up_to,
            day_row,
            day,
            calendar,
        }
    }

    /// Up to the end of the day on the calendar, where the reader is.
    fn up_to_day(&self, zone: &str) -> Option<Range> {
        let date = self.calendar.date();
        Range::up_to(date.year(), date.month(), date.day_of_month(), zone)
    }

    /// The range the choice means now.
    fn range(&self, zone: &str) -> Option<Range> {
        if self.everything.is_active() {
            Some(Range::default())
        } else {
            self.up_to_day(zone)
        }
    }
}

/// One choice from a set: a check button in a radio group, named by its row.
fn choice_row(check: &gtk::CheckButton, title: &str, hint: Option<&str>) -> adw::ActionRow {
    let row = named_row(title);
    if let Some(hint) = hint {
        row.set_subtitle(hint);
    }
    check.set_valign(gtk::Align::Center);
    row.add_prefix(check);
    row.set_activatable_widget(Some(check));
    row
}

impl Sheet {
    fn connect_range(self: &Rc<Self>) {
        let sheet = Rc::downgrade(self);
        self.range.everything.connect_toggled(move |check| {
            if check.is_active()
                && let Some(sheet) = live(&sheet)
            {
                sheet.range.day_row.set_visible(false);
                sheet.choose_range();
            }
        });
        let sheet = Rc::downgrade(self);
        self.range.up_to.connect_toggled(move |check| {
            if check.is_active()
                && let Some(sheet) = live(&sheet)
            {
                sheet.range.day_row.set_visible(true);
                sheet.choose_range();
            }
        });
        let sheet = Rc::downgrade(self);
        self.range.calendar.connect_day_selected(move |_| {
            if let Some(sheet) = live(&sheet) {
                sheet.range.day.popdown();
                sheet.show_day();
                if sheet.range.up_to.is_active() {
                    sheet.choose_range();
                }
            }
        });
        self.show_day();
    }

    /// Puts the chosen day on its button, in the reader's own calendar.
    fn show_day(&self) {
        let label = self
            .range
            .up_to_day(&self.zone)
            .and_then(|range| range.until)
            .and_then(|until| timestamps::local_day(until, &self.zone, l10n::active_locale()))
            .unwrap_or_default();
        self.range.day.set_label(&label);
    }

    fn choose_range(self: &Rc<Self>) {
        let Some(range) = self.range.range(&self.zone) else {
            return;
        };
        let request = self.flow.borrow_mut().choose_range(range);
        self.render();
        if let Some(request) = request {
            self.read(request);
        }
    }

    fn choose_account(self: &Rc<Self>, account: String) {
        let request = self.flow.borrow_mut().choose_account(account);
        self.render();
        if let Some(request) = request {
            self.read(request);
        }
    }

    /// Reads the range on the device. Nothing leaves it.
    fn read(self: &Rc<Self>, request: ReportRequest) {
        let app = Arc::clone(&self.ctx.app);
        let ReportRequest {
            ticket,
            account,
            range,
        } = request;
        let sheet = Rc::downgrade(self);
        off_main_thread(
            move || app.sent_corpus_report(account, range.since, range.until),
            move |report| {
                if let Some(sheet) = live(&sheet) {
                    sheet.flow.borrow_mut().report_arrived(ticket, report);
                    sheet.render();
                }
            },
        );
    }

    /// Pressing Learn under the consent: this, and nothing before it, sends the sample.
    fn learn(self: &Rc<Self>) {
        let Some(request) = self.flow.borrow_mut().consent() else {
            return;
        };
        self.render();
        let app = Arc::clone(&self.ctx.app);
        let name = l10n::writing_style_default_name().to_owned();
        let language = l10n::active_locale().to_owned();
        let sheet = Rc::downgrade(self);
        off_main_thread(
            move || {
                app.learn_writing_style(
                    request.account,
                    request.range.since,
                    request.range.until,
                    name,
                    language,
                )
            },
            move |result| {
                if let Some(sheet) = live(&sheet) {
                    sheet.learned(result.map(|report| report.style_id));
                }
            },
        );
    }

    fn learned(self: &Rc<Self>, result: Result<String, WritingStyleFailure>) {
        self.flow.borrow_mut().finished(result);
        let learned = self.flow.borrow().learned().map(str::to_owned);
        if let Some(style) = learned {
            self.leave();
            (self.on_learned)(style);
        } else {
            self.render();
        }
    }

    /// Takes the progress a snapshot carried. The page forwards every one; only this run's moves
    /// the bar.
    pub(super) fn progress(&self, learning: Option<&LearningProgress>) {
        if self.left.get() {
            return;
        }
        self.flow.borrow_mut().progress(learning);
        let flow = self.flow.borrow();
        if let (LearnStage::Learning(shown), Some(running)) =
            (flow.stage(), self.running.borrow().as_ref())
        {
            running.show(shown.as_ref());
        }
    }

    /// Leaves the sheet. A run still going is stopped: Cancel means the learning too.
    fn leave(&self) {
        self.left.set(true);
        if self.flow.borrow().is_learning() {
            self.ctx.app.cancel_writing_style_learning();
        }
        // On the next turn: the button that asked is inside the detail being removed.
        let navigation = self.ctx.navigation.downgrade();
        glib::idle_add_local_once(move || {
            let Some(navigation) = navigation.upgrade() else {
                return;
            };
            if navigation.visible_child_name().as_deref() == Some(NAME) {
                navigation.set_visible_child_name("settings");
            }
            if let Some(detail) = navigation.child_by_name(NAME) {
                navigation.remove(&detail);
            }
        });
    }

    fn render(self: &Rc<Self>) {
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
        self.running.replace(None);
        let flow = self.flow.borrow();
        self.range
            .group
            .set_visible(*flow.stage() != LearnStage::Account);
        self.range.group.set_sensitive(flow.range_open());
        match flow.stage() {
            LearnStage::Account => self.body.append(&self.account_question()),
            LearnStage::Reading => self.body.append(&Running::new(None, None).root),
            LearnStage::Report(report) => self.show_report(report),
            LearnStage::Learning(progress) => self.show_learning(progress.as_ref()),
            LearnStage::Failed(failure) => {
                self.body.append(&heading(l10n::learn_failed_title()));
                self.body
                    .append(&paragraph(&failure_text(failure, self.route)));
            }
            LearnStage::Learned(_) => {}
        }
    }

    fn account_question(self: &Rc<Self>) -> adw::PreferencesGroup {
        let section = group_titled(l10n::learn_account_title());
        for (id, email) in &self.accounts {
            let row = named_row(email);
            row.set_activatable(true);
            row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
            let sheet = Rc::downgrade(self);
            let id = id.clone();
            // After this handler returns: the answer redraws the list this row is in.
            row.connect_activated(move |_| {
                let (sheet, id) = (sheet.clone(), id.clone());
                glib::idle_add_local_once(move || {
                    if let Some(sheet) = live(&sheet) {
                        sheet.choose_account(id);
                    }
                });
            });
            section.add(&row);
        }
        section
    }

    fn show_report(self: &Rc<Self>, report: &CorpusReport) {
        let facts = adw::PreferencesGroup::new();
        for line in report_lines(report, &self.zone, l10n::active_locale()) {
            facts.add(&named_row(&line));
        }
        self.body.append(&facts);
        if report.usable == 0 {
            self.body.append(&paragraph(l10n::learn_report_nothing()));
            return;
        }
        let Some(consent) = consent_text(self.route, self.own_endpoint.as_deref()) else {
            return;
        };
        self.body.append(&heading(l10n::learn_consent_title()));
        self.body.append(&paragraph(&consent));
        let learn = gtk::Button::with_label(l10n::learn_consent_confirm());
        learn.add_css_class("suggested-action");
        learn.set_halign(gtk::Align::Start);
        let sheet = Rc::downgrade(self);
        // After this handler returns, for the reason the account rows give.
        learn.connect_clicked(move |_| {
            let sheet = sheet.clone();
            glib::idle_add_local_once(move || {
                if let Some(sheet) = live(&sheet) {
                    sheet.learn();
                }
            });
        });
        self.body.append(&learn);
    }

    fn show_learning(&self, progress: Option<&LearningProgress>) {
        let stop = gtk::Button::with_label(l10n::learn_stop());
        let app = Arc::clone(&self.ctx.app);
        stop.connect_clicked(move |button| {
            button.set_sensitive(false);
            app.cancel_writing_style_learning();
        });
        let running = Running::new(progress, Some(&stop));
        self.body.append(&running.root);
        self.running.replace(Some(running));
    }
}

/// A status line with a spinner, or with a bar once there is a count to fill it from.
struct Running {
    root: gtk::Box,
    status: gtk::Label,
    spinner: gtk::Spinner,
    bar: gtk::ProgressBar,
}

impl Running {
    fn new(progress: Option<&LearningProgress>, stop: Option<&gtk::Button>) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let spinner = gtk::Spinner::new();
        let status = paragraph("");
        status.set_hexpand(true);
        line.append(&spinner);
        line.append(&status);
        root.append(&line);
        let bar = gtk::ProgressBar::new();
        root.append(&bar);
        if let Some(stop) = stop {
            stop.set_halign(gtk::Align::Start);
            root.append(stop);
        }
        let running = Self {
            root,
            status,
            spinner,
            bar,
        };
        running.show(progress);
        running
    }

    fn show(&self, progress: Option<&LearningProgress>) {
        let (text, fraction) = progress_text(progress);
        self.status.set_text(&text);
        self.spinner.set_visible(fraction.is_none());
        self.spinner.set_spinning(fraction.is_none());
        self.bar.set_visible(fraction.is_some());
        self.bar.set_fraction(fraction.unwrap_or_default());
    }
}

fn heading(text: &str) -> gtk::Label {
    let label = paragraph(text);
    label.add_css_class("heading");
    label
}
