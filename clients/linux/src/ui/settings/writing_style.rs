//! Settings → Writing style (`docs/ai.md` rules 8 to 16): the page, an existing style's detail in
//! its place, and the flow host over the window's detail for learning a style and for a new
//! style's reveal. The category exists only while the core offers the page.
//!
//! It follows each `Surface::WritingStyle` and `Surface::Settings` signal: the page is drawn again
//! in place when what it reads differs from what it drew, and waits while a picker's list is open
//! or a field holds what it has not sent.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::glib;
use mailcal_bindings::{
    AiIntent, AiRemedy, Intent, StyleLearningIntent, WritingStyleChangeKind, WritingStyleDetail,
    WritingStyleIntent, WritingStyleOpens, WritingStylePage, WritingStyleResult,
};

use super::{
    Category, PageContext, page_box, redraw, writing_style_detail as detail,
    writing_style_learning::{Learning, Wiring},
    writing_style_page as page,
    writing_style_reveal::Reveal,
    writing_style_words as words,
};
use crate::{
    l10n,
    ui::{AppInput, controls, flow_host::FlowHost},
};

#[cfg(test)]
#[path = "writing_style_tests.rs"]
pub(crate) mod tests;

/// The navigation child the flow host is drawn in.
const FLOW: &str = "writing-style-flow";

pub(super) fn writing_style(ctx: &PageContext) -> gtk::Box {
    let content = page_box(l10n::settings_category_writing_style());
    let body = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.append(&body);
    let view = Rc::new(View {
        ctx: ctx.clone(),
        body,
        drawn: RefCell::new(None),
        waiting_on: RefCell::new(None),
        detail: RefCell::new(None),
        language: Cell::new(0),
        seen: Cell::new(0),
        refused: RefCell::new(None),
        learning: RefCell::new(None),
        reveal: RefCell::new(None),
        closing: RefCell::new(None),
    });
    let page = ctx.app.writing_style_page();
    view.seen.set(
        page.as_ref()
            .and_then(|page| page.outcome.as_ref())
            .map_or(0, |outcome| outcome.serial),
    );
    view.draw_page(page);
    let weak = Rc::downgrade(&view);
    let handler = ctx.window.connect_close_request(move |_| {
        if let Some(view) = weak.upgrade() {
            view.leave_flows();
        }
        glib::Propagation::Proceed
    });
    view.closing.replace(Some(handler));
    // The flows are drawn once the window holds its pages: the host goes over them.
    let weak = Rc::downgrade(&view);
    glib::idle_add_local_once(move || {
        if let Some(view) = weak.upgrade() {
            view.follow();
        }
    });
    let following = Rc::clone(&view);
    ctx.writing_style.add(move || following.follow());
    content
}

/// What the page was drawn from: the page, and the detail in its place.
type Drawn = (Option<WritingStylePage>, Option<WritingStyleDetail>);

struct View {
    ctx: PageContext,
    body: gtk::Box,
    drawn: RefCell<Option<Drawn>>,
    waiting_on: RefCell<Option<glib::WeakRef<gtk::Popover>>>,
    /// The style whose detail is in place of the list.
    detail: RefCell<Option<String>>,
    /// Which of its languages the detail shows.
    language: Cell<usize>,
    /// The newest outcome on the page acted on, and what a refusal among them said.
    seen: Cell<u64>,
    refused: RefCell<Option<String>>,
    learning: RefCell<Option<Rc<Learning>>>,
    /// A new style's reveal opened from its row, and the host it is in.
    reveal: RefCell<Option<(Rc<FlowHost>, Rc<Reveal>)>>,
    closing: RefCell<Option<glib::SignalHandlerId>>,
}

impl Drop for View {
    fn drop(&mut self) {
        if let Some(handler) = self.closing.take() {
            self.ctx.window.disconnect(handler);
        }
        title_bar(&self.ctx.window, true);
    }
}

/// Shows or hides `window`'s own title bar.
fn title_bar(window: &gtk::Window, shown: bool) {
    if let Some(bar) = window.titlebar() {
        bar.set_visible(shown);
    }
}

impl View {
    fn follow(self: &Rc<Self>) {
        if !self.ctx.window.is_visible() {
            return;
        }
        let page = self.ctx.app.writing_style_page();
        let outcome = page.as_ref().and_then(|page| page.outcome.clone());
        self.follow_learning(outcome.as_ref());
        self.follow_reveal(outcome.as_ref());
        if let Some(outcome) = outcome.filter(|outcome| outcome.serial > self.seen.get()) {
            self.seen.set(outcome.serial);
            self.refused.replace(match outcome.result {
                WritingStyleResult::Refused { reason }
                    if outcome.kind != WritingStyleChangeKind::SaveReveal =>
                {
                    Some(words::refusal(reason).to_owned())
                }
                _ => None,
            });
            // What was said belongs to the read it came with, so the page is drawn again.
            self.drawn.replace(None);
        }
        let read = (page.clone(), self.read_detail());
        if self.drawn.borrow().as_ref() == Some(&read) {
            return;
        }
        if let Some(open) = redraw::open_popover(self.body.upcast_ref()) {
            self.wait_for(&open);
            return;
        }
        if controls::holds_unsent(self.body.upcast_ref()) {
            return;
        }
        self.draw_page(page);
    }

    fn read_detail(&self) -> Option<WritingStyleDetail> {
        let id = self.detail.borrow().clone()?;
        let found = self.ctx.app.writing_style_detail(id);
        if found.is_none() {
            self.detail.replace(None);
        }
        found
    }

    fn wait_for(self: &Rc<Self>, open: &gtk::Popover) {
        let mut waiting = self.waiting_on.borrow_mut();
        if waiting.as_ref().and_then(glib::WeakRef::upgrade).as_ref() == Some(open) {
            return;
        }
        let view = Rc::downgrade(self);
        open.connect_closed(move |_| {
            let view = view.clone();
            glib::idle_add_local_once(move || {
                if let Some(view) = view.upgrade() {
                    view.follow();
                }
            });
        });
        *waiting = Some(open.downgrade());
    }

    fn draw_page(self: &Rc<Self>, page: Option<WritingStylePage>) {
        while let Some(child) = self.body.first_child() {
            self.body.remove(&child);
        }
        let shown = self.read_detail();
        let refused = self.refused.borrow().clone();
        if let Some(page) = &page {
            if let Some(shown) = &shown {
                let actions = self.detail_actions(shown);
                detail::draw(
                    &self.body,
                    shown,
                    self.language.get(),
                    refused.as_deref(),
                    &actions,
                );
            } else {
                page::draw(&self.body, page, refused.as_deref(), &self.page_actions());
            }
        }
        self.drawn.replace(Some((page, shown)));
    }

    fn send(&self, intent: WritingStyleIntent) {
        self.ctx.app.dispatch(Intent::Ai {
            intent: AiIntent::WritingStyle { intent },
        });
    }

    fn page_actions(self: &Rc<Self>) -> page::Actions {
        let (open, assign, learn, stop, remedy) = (
            Rc::downgrade(self),
            Rc::downgrade(self),
            Rc::downgrade(self),
            Rc::downgrade(self),
            Rc::downgrade(self),
        );
        page::Actions {
            open: Rc::new(move |id, opens| {
                if let Some(view) = open.upgrade() {
                    view.open(id, opens);
                }
            }),
            assign: Rc::new(move |account, style| {
                if let Some(view) = assign.upgrade() {
                    view.send(WritingStyleIntent::Assign { account, style });
                }
            }),
            learn: Rc::new(move || {
                if let Some(view) = learn.upgrade() {
                    view.learning_intent(StyleLearningIntent::Open);
                }
            }),
            stop: Rc::new(move || {
                if let Some(view) = stop.upgrade() {
                    view.learning_intent(StyleLearningIntent::Stop);
                }
            }),
            remedy: Rc::new(move |chosen| {
                if let Some(view) = remedy.upgrade() {
                    view.open_remedy(chosen);
                }
            }),
        }
    }

    fn detail_actions(self: &Rc<Self>, shown: &WritingStyleDetail) -> detail::Actions {
        let id = shown.id.clone();
        let (back, rename, notes, forget, language) = (
            Rc::downgrade(self),
            Rc::downgrade(self),
            Rc::downgrade(self),
            Rc::downgrade(self),
            Rc::downgrade(self),
        );
        let (rename_id, notes_id) = (id.clone(), id);
        let (name, accounts) = (shown.name.clone(), shown.drafting_accounts.clone());
        let forget_id = shown.id.clone();
        detail::Actions {
            back: Rc::new(move || {
                if let Some(view) = back.upgrade() {
                    view.detail.replace(None);
                    view.refused.replace(None);
                    view.draw_page(view.ctx.app.writing_style_page());
                }
            }),
            rename: Rc::new(move |typed| {
                if let Some(view) = rename.upgrade() {
                    view.send(WritingStyleIntent::Rename {
                        style: rename_id.clone(),
                        name: typed,
                    });
                }
            }),
            notes: Rc::new(move |typed| {
                if let Some(view) = notes.upgrade() {
                    view.send(WritingStyleIntent::SetNotes {
                        style: notes_id.clone(),
                        notes: typed,
                    });
                }
            }),
            forget: Rc::new(move || {
                if let Some(view) = forget.upgrade() {
                    view.confirm_forget(&forget_id, &name, &accounts);
                }
            }),
            language: Rc::new(move |index| {
                if let Some(view) = language.upgrade() {
                    view.language.set(index);
                    view.draw_page(view.ctx.app.writing_style_page());
                }
            }),
        }
    }

    /// Asks before forgetting, naming the accounts that draft in the style.
    fn confirm_forget(self: &Rc<Self>, id: &str, name: &str, accounts: &[String]) {
        let mut message = l10n::writing_style_forget_message().to_owned();
        if !accounts.is_empty() {
            message.push_str("\n\n");
            message.push_str(&l10n::writing_style_forget_accounts(&accounts.join(", ")));
        }
        let (view, id) = (Rc::downgrade(self), id.to_owned());
        controls::confirm(
            &self.ctx.window,
            &l10n::writing_style_forget_title(name),
            &message,
            l10n::writing_style_forget(),
            move |confirmed| {
                if confirmed && let Some(view) = view.upgrade() {
                    view.send(WritingStyleIntent::Forget { style: id.clone() });
                }
            },
        );
    }

    fn open(self: &Rc<Self>, id: String, opens: WritingStyleOpens) {
        match opens {
            WritingStyleOpens::Detail => {
                self.detail.replace(Some(id));
                self.language.set(0);
                self.refused.replace(None);
                self.draw_page(self.ctx.app.writing_style_page());
            }
            WritingStyleOpens::Reveal => {
                let Some(record) = self.ctx.app.writing_style_reveal(id) else {
                    return;
                };
                let view = Rc::downgrade(self);
                let send: super::writing_style_reveal::Send = Rc::new(move |intent| {
                    if let Some(view) = view.upgrade() {
                        view.send(intent);
                    }
                });
                let reveal = Reveal::new(record, self.seen.get(), Rc::clone(&send));
                let (closing, id) = (Rc::clone(&send), reveal.id.clone());
                let host = FlowHost::new(
                    Rc::new(|| {}),
                    Rc::new(move || closing(WritingStyleIntent::CloseReveal { style: id.clone() })),
                );
                let (weak, shown) = (Rc::downgrade(&reveal), Rc::downgrade(&host));
                reveal.on_reshow(Rc::new(move || {
                    if let (Some(reveal), Some(host)) = (weak.upgrade(), shown.upgrade()) {
                        host.show(&[], reveal.step());
                    }
                }));
                host.show(&[], reveal.step());
                self.present(host.widget().upcast_ref());
                self.reveal.replace(Some((host, reveal)));
            }
        }
    }

    fn follow_reveal(self: &Rc<Self>, outcome: Option<&mailcal_bindings::WritingStyleOutcome>) {
        let Some((host, reveal)) = self.reveal.borrow().clone() else {
            return;
        };
        match self.ctx.app.writing_style_reveal(reveal.id.clone()) {
            Some(record) if !reveal.outcome(outcome) => {
                reveal.update(record);
                host.show(&[], reveal.step());
            }
            _ => {
                host.dismiss();
                self.reveal.replace(None);
                self.dismiss(host.widget().upcast_ref());
            }
        }
    }

    fn follow_learning(self: &Rc<Self>, outcome: Option<&mailcal_bindings::WritingStyleOutcome>) {
        let record = self.ctx.app.writing_style_learning();
        let current = self.learning.borrow().clone();
        match (record, current) {
            (Some(record), Some(flow)) => flow.draw(record, outcome),
            (Some(record), None) => {
                let flow = Learning::new(self.wiring());
                flow.draw(record, outcome);
                self.present(flow.host.widget().upcast_ref());
                self.learning.replace(Some(flow));
            }
            (None, Some(flow)) => {
                flow.host.dismiss();
                self.learning.replace(None);
                self.dismiss(flow.host.widget().upcast_ref());
            }
            (None, None) => {}
        }
    }

    fn wiring(self: &Rc<Self>) -> Wiring {
        let (send, style, reveal_of, remedy) = (
            Rc::downgrade(self),
            Rc::downgrade(self),
            std::sync::Arc::clone(&self.ctx.app),
            Rc::downgrade(self),
        );
        Wiring {
            send: Rc::new(move |intent| {
                if let Some(view) = send.upgrade() {
                    view.learning_intent(intent);
                }
            }),
            style: Rc::new(move |intent| {
                if let Some(view) = style.upgrade() {
                    view.send(intent);
                }
            }),
            reveal_of: Rc::new(move |id| reveal_of.writing_style_reveal(id)),
            open_remedy: Rc::new(move |chosen| {
                if let Some(view) = remedy.upgrade() {
                    view.open_remedy(chosen);
                }
            }),
        }
    }

    fn learning_intent(&self, intent: StyleLearningIntent) {
        self.send(WritingStyleIntent::Learning { intent });
    }

    /// Opens what a remedy names that lives elsewhere in Settings.
    fn open_remedy(&self, remedy: AiRemedy) {
        match remedy {
            AiRemedy::SignInToAllodia => {
                self.ctx.sidebar.show(Category::Allodia);
                self.ctx.sender.emit(AppInput::StartAllodiaSignIn);
            }
            AiRemedy::OpenAiEndpoint => self.ctx.sidebar.show(Category::Advanced),
            // A step of the flow, from a run that failed with its host closed: the flow opens on
            // the failure and takes it.
            AiRemedy::TryAgain | AiRemedy::ChooseAccount | AiRemedy::ChooseRange => {
                self.learning_intent(StyleLearningIntent::Open);
                self.learning_intent(StyleLearningIntent::Remedy);
            }
            AiRemedy::LearnStyle | AiRemedy::ChooseStyle => {}
        }
    }

    /// Closing the window closes a flow on it, as Cancel would.
    fn leave_flows(&self) {
        if let Some(flow) = self.learning.borrow().as_ref() {
            flow.close();
        }
        if let Some((_, reveal)) = self.reveal.borrow().as_ref() {
            self.send(WritingStyleIntent::CloseReveal {
                style: reveal.id.clone(),
            });
        }
    }

    /// Puts the flow host over the window's detail.
    fn present(&self, host: &gtk::Widget) {
        let navigation = &self.ctx.navigation;
        if let Some(previous) = navigation.child_by_name(FLOW) {
            navigation.remove(&previous);
        }
        navigation.add_named(host, Some(FLOW));
        navigation.set_visible_child_name(FLOW);
        // The flow's header bar stands in for the window's own while it is up: one bar, with the
        // window's controls at its end.
        title_bar(&self.ctx.window, false);
    }

    /// Puts the Settings pages back in place of the host, on the next turn of the loop: the call
    /// may come from a button inside it.
    fn dismiss(&self, host: &gtk::Widget) {
        let (navigation, host) = (self.ctx.navigation.downgrade(), host.downgrade());
        glib::idle_add_local_once(move || {
            let (Some(navigation), Some(host)) = (navigation.upgrade(), host.upgrade()) else {
                return;
            };
            if navigation.child_by_name(FLOW).as_ref() != Some(&host) {
                return;
            }
            navigation.set_visible_child_name("settings");
            navigation.remove(&host);
            if let Some(window) = navigation.root().and_downcast::<gtk::Window>() {
                title_bar(&window, true);
            }
        });
    }
}
