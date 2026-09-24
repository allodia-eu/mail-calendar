//! The sheet that learns a writing style (`docs/ai.md`, "Learning"): which account when there is
//! a choice, which sent mail, what that holds on this device with the consent beneath, and the run,
//! one page each in the frame the reveal uses.
//!
//! A detail inside the Settings window, as the signature editor is, for its reason: a second
//! toplevel races GTK's accessibility bridge (`docs/signatures.md`). The report is read again
//! whenever the account or the range changes, so what the consent says is always about the mail
//! Learn would send; pressing Learn under it is the consent. Where the sheet stands is
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

use super::{
    PageContext,
    signatures::named_row,
    wizard::{Wizard, WizardPage},
    writing_style::paragraph,
    writing_style_learn_parts::{RangeChoice, Running, choice_row, heading},
};
use crate::{
    l10n,
    ui::{
        blocking::off_main_thread,
        timestamps,
        writing_style::failure_text,
        writing_style_flow::{
            LearnAction, LearnFlow, LearnPage, LearnStage, ReportRequest, consent_text,
            report_lines,
        },
        writing_style_reveal::WizardPager,
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
    wizard: Wizard,
    pages: Vec<LearnPage>,
    pager: Cell<WizardPager>,
    range: RangeChoice,
    /// The report and the consent, redrawn whenever the stage changes.
    consent: WizardPage,
    /// The run, and how it ended.
    progress: WizardPage,
    /// A running learn's status, updated in place as progress arrives.
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
    let pages = LearnPage::all(accounts.len());
    let pager = WizardPager::new(pages.len());
    let wizard = Wizard::new(l10n::writing_style_learn());
    let range = RangeChoice::new();
    let consent = WizardPage::new(l10n::writing_style_learn());
    let progress = WizardPage::new("");
    progress.title.set_visible(false);
    let sheet = Rc::new(Sheet {
        ctx: ctx.clone(),
        flow: RefCell::new(flow),
        route: snapshot.route,
        own_endpoint: ctx.app.own_ai_endpoint().map(|endpoint| endpoint.base_url),
        zone: ctx.app.timezone_settings().active,
        accounts,
        wizard,
        pages,
        pager: Cell::new(pager),
        range,
        consent,
        progress,
        running: RefCell::new(None),
        left: Cell::new(false),
        on_learned: Box::new(on_learned),
    });
    sheet.build();
    ctx.navigation.add_named(&sheet.wizard.root, Some(NAME));
    ctx.navigation.set_visible_child_name(NAME);
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

impl Sheet {
    fn build(self: &Rc<Self>) {
        for page in &self.pages {
            match page {
                LearnPage::Account => {
                    let account = WizardPage::new(l10n::learn_account_title());
                    account.body.append(&self.account_question());
                    self.wizard.append(&account);
                }
                LearnPage::Range => {
                    let range = WizardPage::new(l10n::learn_range_title());
                    range.body.append(&self.range.group);
                    self.wizard.append(&range);
                }
                LearnPage::Consent => self.wizard.append(&self.consent),
                LearnPage::Progress => self.wizard.append(&self.progress),
            }
        }
        self.connect_range();
        let sheet = Rc::downgrade(self);
        self.wizard.cancel.connect_clicked(move |_| {
            if let Some(sheet) = live(&sheet) {
                sheet.leave();
            }
        });
        let sheet = Rc::downgrade(self);
        self.wizard.back.connect_clicked(move |_| {
            if let Some(sheet) = live(&sheet) {
                sheet.turn(WizardPager::back);
            }
        });
        let sheet = Rc::downgrade(self);
        self.wizard.primary.connect_clicked(move |button| {
            if let Some(sheet) = live(&sheet) {
                sheet.primary(button);
            }
        });
    }

    fn page(&self) -> LearnPage {
        self.pages[self.pager.get().index()]
    }

    fn turn(&self, move_to: fn(&mut WizardPager)) {
        let mut pager = self.pager.get();
        move_to(&mut pager);
        self.pager.set(pager);
        self.render();
    }

    /// Next, Learn, Stop or Close: what stands there is the flow's answer for the page on screen.
    fn primary(self: &Rc<Self>, button: &gtk::Button) {
        let action = self.flow.borrow().action(self.page());
        match action {
            LearnAction::Next { enabled: true } => self.turn(WizardPager::next),
            LearnAction::Learn => self.learn(),
            LearnAction::Stop => {
                button.set_sensitive(false);
                self.ctx.app.cancel_writing_style_learning();
            }
            LearnAction::Close => self.leave(),
            LearnAction::Next { enabled: false } | LearnAction::Nothing => {}
        }
    }

    fn account_question(self: &Rc<Self>) -> adw::PreferencesGroup {
        let section = adw::PreferencesGroup::new();
        let mut first: Option<gtk::CheckButton> = None;
        for (id, email) in &self.accounts {
            let check = gtk::CheckButton::new();
            if let Some(first) = &first {
                check.set_group(Some(first));
            } else {
                first = Some(check.clone());
            }
            let sheet = Rc::downgrade(self);
            let id = id.clone();
            check.connect_toggled(move |check| {
                if check.is_active()
                    && let Some(sheet) = live(&sheet)
                {
                    sheet.choose_account(id.clone());
                }
            });
            section.add(&choice_row(&check, email, None));
        }
        section
    }

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

    /// Pressing Learn under the consent: this, and nothing before it, sends the sample. The sheet
    /// turns to the run's page, which only this reaches.
    fn learn(self: &Rc<Self>) {
        let Some(request) = self.flow.borrow_mut().consent() else {
            return;
        };
        let mut pager = self.pager.get();
        pager.go(self.pages.len() - 1);
        self.pager.set(pager);
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

    /// Leaves the sheet. A run still going is stopped: leaving means the learning too.
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

    /// Brings the frame to the page on screen, and redraws the two pages whose content is the
    /// flow's stage.
    fn render(&self) {
        let pager = self.pager.get();
        let page = self.page();
        self.wizard.show(pager);
        self.wizard.cancel.set_visible(page.can_go_back());
        self.wizard
            .back
            .set_visible(page.can_go_back() && !pager.is_first());
        let flow = self.flow.borrow();
        self.range.group.set_sensitive(flow.range_open());
        match flow.action(page) {
            LearnAction::Next { enabled } => {
                self.wizard.set_primary(l10n::wizard_next(), true, enabled);
            }
            LearnAction::Learn => {
                self.wizard
                    .set_primary(l10n::learn_consent_confirm(), true, true);
            }
            LearnAction::Stop => self.wizard.set_primary(l10n::learn_stop(), false, true),
            LearnAction::Close => self.wizard.set_primary(l10n::action_close(), true, true),
            LearnAction::Nothing => self.wizard.primary.set_visible(false),
        }
        if page == LearnPage::Progress {
            self.show_run(flow.stage());
        } else {
            self.show_consent(flow.stage());
        }
    }

    /// What the chosen mail holds and what Learn would send where. A run's own stages are the
    /// progress page's.
    fn show_consent(&self, stage: &LearnStage) {
        let body = &self.consent.body;
        match stage {
            LearnStage::Account => {
                self.consent.clear();
            }
            LearnStage::Reading => {
                self.consent.clear();
                body.append(&Running::new(None).root);
            }
            LearnStage::Report(report) => {
                self.consent.clear();
                self.show_report(report);
            }
            LearnStage::Failed(failure) => {
                self.consent.clear();
                body.append(&paragraph(&failure_text(failure, self.route)));
            }
            LearnStage::Learning(_) | LearnStage::Learned(_) => {}
        }
    }

    fn show_report(&self, report: &CorpusReport) {
        let body = &self.consent.body;
        let facts = adw::PreferencesGroup::new();
        for line in report_lines(report, &self.zone, l10n::active_locale()) {
            facts.add(&named_row(&line));
        }
        body.append(&facts);
        if report.usable == 0 {
            body.append(&paragraph(l10n::learn_report_nothing()));
            return;
        }
        if let Some(consent) = consent_text(self.route, self.own_endpoint.as_deref()) {
            body.append(&heading(l10n::learn_consent_title()));
            body.append(&paragraph(&consent));
        }
    }

    /// The run, and how it ended when it ended without a style. A style learned leaves the sheet.
    fn show_run(&self, stage: &LearnStage) {
        let title = &self.progress.title;
        match stage {
            LearnStage::Learning(progress) => {
                title.set_visible(false);
                if let Some(running) = self.running.borrow().as_ref() {
                    running.show(progress.as_ref());
                    return;
                }
                self.progress.clear();
                let running = Running::new(progress.as_ref());
                self.progress.body.append(&running.root);
                self.running.replace(Some(running));
            }
            LearnStage::Failed(failure) => {
                self.running.replace(None);
                self.progress.clear();
                title.set_text(l10n::learn_failed_title());
                title.set_visible(true);
                self.progress
                    .body
                    .append(&paragraph(&failure_text(failure, self.route)));
            }
            LearnStage::Account
            | LearnStage::Reading
            | LearnStage::Report(_)
            | LearnStage::Learned(_) => {}
        }
    }
}
