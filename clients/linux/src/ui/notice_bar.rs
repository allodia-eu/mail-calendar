//! The window's one notice banner (`docs/notices.md`): the first notice that belongs to the screen
//! on show, in an `AdwBanner` across the top, and every other one behind an Issues button beside
//! it. Never a second banner. Which notice and how many is [`super::notice_rules`]; this half turns
//! that into widgets and an announcement.
//!
//! An `AdwBanner` holds a sentence and one button and nothing else, so the strip around it carries
//! what the banner cannot: the severity's glyph before it, a spinner beside its button while that
//! button's sign-in runs (`docs/waiting.md` rule 1), and the Issues button after it. The strip, not
//! the banner, is tinted, so the three read as one bar.
//!
//! A question (rule 8) has two answers, and its banner draws neither: the strip draws both after
//! the sentence, the lesser first, because a banner's own button can be neither held nor given a
//! sibling. While the core carries an answer out, both are held and the primary's spins.

use std::{cell::RefCell, rc::Rc, sync::Once};

use adw::prelude::*;
use gtk::accessible::Property as AccessibleProperty;
use mailcal_bindings::{ControlState, Notice, NoticeScreens, NoticeSeverity};

use super::{
    AppInput, PrimaryView,
    notice_rules::{self, Announcements},
    notices::NoticeInput,
};
use crate::l10n;

const CSS: &str = "
.notice-strip { background-color: color-mix(in srgb, var(--accent-bg-color) 30%, transparent); }
.notice-strip.blocked { background-color: color-mix(in srgb, var(--error-bg-color) 30%, transparent); }
.notice-strip banner > revealer > widget { background-color: transparent; }
";

/// What the strip was drawn from, so a render that changed none of it leaves it alone.
#[derive(PartialEq)]
struct Drawn {
    notices: Vec<Notice>,
    screen: NoticeScreens,
    running: Option<String>,
}

pub(super) struct NoticeBar {
    root: gtk::Box,
    glyph: gtk::Image,
    banner: adw::Banner,
    /// A question's two answers, in place of the banner's button.
    answers: gtk::Box,
    spinner: adw::Spinner,
    issues: gtk::MenuButton,
    list: gtk::Box,
    /// The notice the banner holds, which its button acts on.
    shown: Rc<RefCell<Option<Notice>>>,
    drawn: Option<Drawn>,
    announced: Announcements,
    sender: relm4::Sender<AppInput>,
}

impl NoticeBar {
    pub(super) fn new(sender: &relm4::Sender<AppInput>) -> Self {
        install_styles();
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        root.add_css_class("notice-strip");
        root.set_visible(false);
        let glyph = gtk::Image::new();
        glyph.set_margin_start(12);
        // The sentence beside it says what the glyph shows.
        glyph.set_accessible_role(gtk::AccessibleRole::Presentation);
        root.append(&glyph);
        let banner = adw::Banner::new("");
        // A notice names an account by its address and a folder by the server's name: neither is
        // markup.
        banner.set_use_markup(false);
        banner.set_hexpand(true);
        banner.set_revealed(true);
        let shown: Rc<RefCell<Option<Notice>>> = Rc::default();
        let (input, current) = (sender.clone(), Rc::clone(&shown));
        banner.connect_button_clicked(move |_| {
            if let Some(notice) = current.borrow().as_ref() {
                input.emit(AppInput::Notice(button_input(notice)));
            }
        });
        root.append(&banner);
        let answers = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        answers.set_margin_end(12);
        answers.set_visible(false);
        root.append(&answers);
        let spinner = adw::Spinner::new();
        spinner.set_visible(false);
        spinner.set_margin_end(12);
        root.append(&spinner);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 18);
        list.set_margin_top(12);
        list.set_margin_bottom(12);
        list.set_margin_start(12);
        list.set_margin_end(12);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_propagate_natural_height(true);
        scroll.set_max_content_height(480);
        scroll.set_child(Some(&list));
        let popover = gtk::Popover::new();
        popover.set_child(Some(&scroll));
        let issues = gtk::MenuButton::new();
        issues.set_popover(Some(&popover));
        issues.add_css_class("flat");
        issues.set_valign(gtk::Align::Center);
        issues.set_margin_end(6);
        issues.set_visible(false);
        root.append(&issues);
        Self {
            root,
            glyph,
            banner,
            answers,
            spinner,
            issues,
            list,
            shown,
            drawn: None,
            announced: Announcements::default(),
            sender: sender.clone(),
        }
    }

    pub(super) fn widget(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    pub(super) fn render(
        &mut self,
        notices: &[Notice],
        primary: PrimaryView,
        running: Option<&str>,
    ) {
        let drawn = Drawn {
            notices: notices.to_vec(),
            screen: notice_rules::screen_of(primary),
            running: running.map(str::to_owned),
        };
        if self.drawn.as_ref() == Some(&drawn) {
            return;
        }
        if self
            .drawn
            .as_ref()
            .is_none_or(|before| before.notices != drawn.notices)
        {
            self.announce(notices);
        }
        self.draw(&drawn);
        self.drawn = Some(drawn);
    }

    fn draw(&self, drawn: &Drawn) {
        let notices = &drawn.notices;
        let shown = notice_rules::prominent(notices, drawn.screen).cloned();
        self.shown.replace(shown.clone());
        let Some(notice) = shown else {
            self.root.set_visible(false);
            self.issues.popdown();
            return;
        };
        if notice.severity == NoticeSeverity::Blocked {
            self.root.add_css_class("blocked");
        } else {
            self.root.remove_css_class("blocked");
        }
        self.glyph
            .set_icon_name(Some(notice_rules::glyph(notice.severity)));
        self.banner.set_title(&sentence(&notice));
        let running = drawn.running.as_deref() == Some(notice.id.as_str());
        let asks = notice_rules::asks(&notice);
        self.banner
            .set_button_label(button_label(&notice).filter(|_| !asks).as_deref());
        self.fill_answers(&notice, asks, running);
        self.spinner.set_visible(running && !asks);
        let more = notice_rules::more(notices, drawn.screen);
        self.issues.set_visible(more > 0);
        let label = l10n::notices_more(i64::try_from(more).unwrap_or(i64::MAX));
        self.issues.set_label(&label);
        self.issues
            .update_property(&[AccessibleProperty::Label(&label)]);
        self.fill_list(notices, drawn.running.as_deref());
        self.root.set_visible(true);
    }

    /// The banner's question's answers, or nothing when the banner holds no question.
    fn fill_answers(&self, notice: &Notice, asks: bool, running: bool) {
        while let Some(child) = self.answers.first_child() {
            self.answers.remove(&child);
        }
        self.answers.set_visible(asks);
        if asks {
            for button in answer_buttons(notice, running, None, &self.sender) {
                self.answers.append(&button);
            }
        }
    }

    /// Every notice in force, each with its own action and, where it can be closed, a Close.
    fn fill_list(&self, notices: &[Notice], running: Option<&str>) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let heading = gtk::Label::new(Some(l10n::notices_issues()));
        heading.add_css_class("heading");
        heading.set_xalign(0.0);
        self.list.append(&heading);
        for notice in notices {
            let running = running == Some(notice.id.as_str());
            self.list
                .append(&issue_row(notice, running, &self.issues, &self.sender));
        }
    }

    /// Says each notice that has just arrived to assistive technology, once (rule 12).
    fn announce(&mut self, notices: &[Notice]) {
        for notice in self.announced.arrived(notices) {
            self.root.announce(
                &notice_rules::words(&notice.title),
                gtk::AccessibleAnnouncementPriority::Medium,
            );
        }
    }
}

/// The banner's one sentence: the notice's, and beneath it what went wrong with its remedy or
/// why. An `AdwBanner` has a title and nothing under it, so the two share it, a line apart.
fn sentence(notice: &Notice) -> String {
    let title = notice_rules::words(&notice.title);
    match notice_rules::beneath(notice) {
        Some(beneath) => format!("{title}\n{beneath}"),
        None => title,
    }
}

/// The banner's button: the notice's action, or Close for one that only closes.
fn button_label(notice: &Notice) -> Option<String> {
    if notice_rules::offers(notice) {
        notice.action_label.as_ref().map(notice_rules::words)
    } else if notice.dismissible {
        Some(l10n::notices_dismiss().to_owned())
    } else {
        None
    }
}

/// What the banner's button asks for, as [`button_label`] named it.
fn button_input(notice: &Notice) -> NoticeInput {
    if notice_rules::offers(notice) {
        NoticeInput::Run(notice.id.clone())
    } else {
        NoticeInput::Dismiss(notice.id.clone())
    }
}

/// One notice in the Issues list: its glyph, its sentence, what is said beneath it and its
/// buttons. Each button closes the list as it acts.
fn issue_row(
    notice: &Notice,
    running: bool,
    issues: &gtk::MenuButton,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let glyph = gtk::Image::from_icon_name(notice_rules::glyph(notice.severity));
    glyph.set_valign(gtk::Align::Start);
    glyph.set_accessible_role(gtk::AccessibleRole::Presentation);
    if notice.severity == NoticeSeverity::Blocked {
        glyph.add_css_class("error");
    }
    row.append(&glyph);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let title = notice_rules::words(&notice.title);
    text.append(&wrapped(&title));
    if let Some(beneath) = notice_rules::beneath(notice) {
        let label = wrapped(&beneath);
        label.add_css_class("dim-label");
        label.add_css_class("caption");
        text.append(&label);
    }
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    if notice_rules::asks(notice) {
        for button in answer_buttons(notice, running, Some(issues), sender) {
            buttons.append(&button);
        }
    } else if let Some(label) = notice
        .action_label
        .as_ref()
        .filter(|_| notice_rules::offers(notice))
    {
        let label = notice_rules::words(label);
        let input = NoticeInput::Run(notice.id.clone());
        let control = ControlState {
            enabled: true,
            running,
        };
        let about = Some(title.as_str());
        buttons.append(&notice_button(
            &label,
            about,
            control,
            input,
            Some(issues),
            sender,
        ));
    }
    if notice.dismissible {
        let input = NoticeInput::Dismiss(notice.id.clone());
        let control = ControlState {
            enabled: true,
            running: false,
        };
        let close = notice_button(
            l10n::notices_dismiss(),
            Some(&title),
            control,
            input,
            Some(issues),
            sender,
        );
        buttons.append(&close);
    }
    if buttons.first_child().is_some() {
        text.append(&buttons);
    }
    row.append(&text);
    row
}

fn wrapped(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_max_width_chars(48);
    label
}

/// A question's two answers, the lesser first. While one is carried out, by the core or by a flow
/// this client started, both are held and the primary's spins (`docs/waiting.md`). In the Issues
/// list (`issues`) each is named with the question and closes the list as it acts.
fn answer_buttons(
    notice: &Notice,
    running: bool,
    issues: Option<&gtk::MenuButton>,
    sender: &relm4::Sender<AppInput>,
) -> Vec<gtk::Button> {
    let held = running || notice.running;
    let title = notice_rules::words(&notice.title);
    let about = issues.map(|_| title.as_str());
    let mut buttons = Vec::with_capacity(2);
    if let Some(label) = &notice.second_action_label {
        let control = ControlState {
            enabled: !held,
            running: false,
        };
        let input = NoticeInput::Second(notice.id.clone());
        let label = notice_rules::words(label);
        let second = notice_button(&label, about, control, input, issues, sender);
        second.add_css_class("flat");
        buttons.push(second);
    }
    if let Some(label) = &notice.action_label {
        let control = ControlState {
            enabled: !held,
            running: held,
        };
        let input = NoticeInput::Run(notice.id.clone());
        let label = notice_rules::words(label);
        buttons.push(notice_button(&label, about, control, input, issues, sender));
    }
    buttons
}

/// A notice's button. In the Issues list it is named with its notice's sentence (`about`), so a
/// screen reader can tell two Sign in again buttons apart, and closes the list as it acts. A
/// running sign-in shows a spinner and stays pressable: a browser sign-in pressed again replaces
/// the one still out (`docs/waiting.md` rule 2).
fn notice_button(
    label: &str,
    about: Option<&str>,
    control: ControlState,
    input: NoticeInput,
    issues: Option<&gtk::MenuButton>,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Button {
    let (sender, issues) = (sender.clone(), issues.cloned());
    let button = super::controls::button(label, control, false, move || {
        if let Some(issues) = &issues {
            issues.popdown();
        }
        sender.emit(AppInput::Notice(input.clone()));
    });
    button.set_halign(gtk::Align::Start);
    if let Some(about) = about {
        button.update_property(&[AccessibleProperty::Label(&format!("{label}: {about}"))]);
    }
    button
}

fn install_styles() {
    static INSTALLED: Once = Once::new();
    if let Some(display) = gtk::gdk::Display::default() {
        INSTALLED.call_once(|| {
            let provider = gtk::CssProvider::new();
            provider.load_from_string(CSS);
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            super::icons::install(&display);
        });
    }
}

#[cfg(test)]
#[path = "notice_bar_tests.rs"]
pub(crate) mod tests;
