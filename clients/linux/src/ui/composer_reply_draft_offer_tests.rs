//! Draft a reply exists in a composer exactly while the core's record does (`docs/ai.md` rules 4
//! and 24): not offered, the composer has no control and no notes, and Send sends without a
//! question; withdrawn while the composer is open, the control, the notes, an open input and an
//! open question go with it, each closed unanswered, and none comes back open with the offer.
//! Called from the crate's single `gtk::init` test through [`super::all`].

use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, Mutex},
};

use adw::prelude::*;
use mailcal_bindings::{
    AiIntent, ComposerReplyDraft, ReplyDraftIntent, ReplyDraftItem, ReplyDraftSendQuestion,
    ReplyDraftState, ReplyDraftTaskKind,
};

use super::{FREE, record};
use crate::ui::{
    AppInput,
    composer_reply_draft::{ReplyDraft, ReplyDraftCore},
    webview::{DocumentKind, SecureWebView},
};

/// The core as far as Draft a reply asks it: the record it is given, and every intent sent.
#[derive(Default)]
struct Core {
    record: Mutex<Option<ComposerReplyDraft>>,
    sent: Mutex<Vec<String>>,
}

impl Core {
    fn offer(&self, record: Option<ComposerReplyDraft>) {
        *self.record.lock().expect("record") = record;
    }

    fn sent(&self) -> Vec<String> {
        self.sent.lock().expect("sent").clone()
    }
}

impl ReplyDraftCore for Core {
    fn record(&self, _: &str, _: Option<String>) -> Option<ComposerReplyDraft> {
        self.record.lock().expect("record").clone()
    }

    fn report(&self, _: &str, _: String) {}

    fn dispatch(&self, intent: AiIntent) {
        let said = match intent {
            AiIntent::ReplyDraft {
                intent: ReplyDraftIntent::SendQuestionAnswered { .. },
            } => "answered".to_owned(),
            AiIntent::ReplyDraft {
                intent: ReplyDraftIntent::AnswerArrived { replace, .. },
            } => format!("arrived {replace}"),
            AiIntent::ReplyDraft {
                intent: ReplyDraftIntent::Draft { replace, .. },
            } => format!("draft {replace}"),
            _ => "another intent".to_owned(),
        };
        self.sent.lock().expect("sent").push(said);
    }
}

fn settle() {
    while gtk::glib::MainContext::default().iteration(false) {}
}

fn drafted() -> ComposerReplyDraft {
    ComposerReplyDraft {
        send_question: Some(ReplyDraftSendQuestion { open: 1 }),
        ..record(ReplyDraftState::Drafted {
            summary: "Asks whether Thursday still works.".to_owned(),
            checklist: vec![ReplyDraftItem {
                index: 0,
                kind: ReplyDraftTaskKind::Do,
                text: "Book the room".to_owned(),
                done: false,
                tick: Some(FREE),
            }],
        })
    }
}

/// A composer's Draft a reply in a window, as the composer holds it: the control in the action
/// row, the notes under it, the editor the questions are asked over.
struct Composer {
    window: adw::Window,
    draft: Rc<ReplyDraft>,
}

impl Composer {
    fn open(core: &Arc<Core>) -> Self {
        let (sender, _receiver) = relm4::channel::<AppInput>();
        // Built as the composer builds it: a bare `webkit6::WebView::new()` takes WebKit's default
        // network session, and the test process then aborts on the way out.
        let editor = SecureWebView::new(DocumentKind::Composer, sender.clone());
        let draft = ReplyDraft::new(
            Arc::clone(core) as Arc<dyn ReplyDraftCore>,
            "composition-1",
            Rc::new(|| Some("acct-1".to_owned())),
            editor.widget(),
            sender,
        );
        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column.append(draft.slot());
        column.append(draft.notes());
        column.append(editor.widget());
        let window = adw::Window::new();
        window.set_content(Some(&column));
        window.present();
        settle();
        Self { window, draft }
    }

    fn follow(&self, core: &Core, record: Option<ComposerReplyDraft>) {
        core.offer(record);
        self.draft.follow();
        settle();
    }

    fn control(&self) -> bool {
        self.draft.slot().is_visible() && self.draft.slot().first_child().is_some()
    }

    fn question(&self) -> bool {
        self.window.visible_dialog().is_some()
    }

    /// Presses Send, counting what it came to: sent, or left as Keep editing leaves it.
    fn send(&self, sent: &Rc<Cell<u32>>, kept: &Rc<Cell<u32>>) {
        let (sent, kept) = (Rc::clone(sent), Rc::clone(kept));
        self.draft.before_send(
            Box::new(move || sent.set(sent.get() + 1)),
            Box::new(move || kept.set(kept.get() + 1)),
        );
        settle();
    }

    fn assert_nothing_drawn(&self) {
        assert!(!self.control(), "no Draft a reply control");
        assert!(!self.draft.notes().is_visible(), "nothing among the notes");
        assert!(!self.draft.popover.is_visible(), "no instruction input");
        assert!(!self.question(), "no question");
    }
}

pub(super) fn not_offered_the_composer_has_none_of_it_and_send_sends() {
    let core = Arc::new(Core::default());
    let composer = Composer::open(&core);
    composer.assert_nothing_drawn();

    let (sent, kept) = (Rc::default(), Rc::default());
    composer.send(&sent, &kept);
    assert_eq!(
        (sent.get(), kept.get()),
        (1, 0),
        "Send sends, asking nothing"
    );
    composer.assert_nothing_drawn();
    assert_eq!(core.sent(), Vec::<String>::new());
    composer.window.destroy();
}

pub(super) fn an_open_input_goes_with_the_offer_and_does_not_come_back_open() {
    let core = Arc::new(Core::default());
    core.offer(Some(drafted()));
    let composer = Composer::open(&core);
    assert!(composer.control());
    assert!(
        composer.draft.notes().is_visible(),
        "the card among the notes"
    );
    composer
        .draft
        .slot()
        .first_child()
        .and_downcast::<gtk::MenuButton>()
        .expect("the control")
        .popup();
    settle();
    assert!(composer.draft.popover.is_visible(), "the input is open");

    composer.follow(&core, None);
    composer.assert_nothing_drawn();

    composer.follow(&core, Some(drafted()));
    assert!(composer.control());
    assert!(
        !composer.draft.popover.is_visible(),
        "the input stays closed"
    );
    assert_eq!(core.sent(), Vec::<String>::new());
    composer.window.destroy();
}

pub(super) fn an_open_send_question_goes_with_the_offer_unanswered() {
    let core = Arc::new(Core::default());
    core.offer(Some(drafted()));
    let composer = Composer::open(&core);
    let (sent, kept) = (Rc::default(), Rc::default());
    composer.send(&sent, &kept);
    assert!(
        composer.question(),
        "asked while the checklist has open items"
    );

    composer.follow(&core, None);
    composer.assert_nothing_drawn();
    assert_eq!(
        (sent.get(), kept.get()),
        (0, 1),
        "nothing sent: the composer is left as Keep editing leaves it"
    );
    assert_eq!(
        core.sent(),
        Vec::<String>::new(),
        "the core is told no answer"
    );

    composer.send(&sent, &kept);
    assert_eq!(sent.get(), 1, "Send then sends");
    composer.window.destroy();
}

pub(super) fn an_open_replace_question_goes_with_the_offer_unanswered() {
    let core = Arc::new(Core::default());
    core.offer(Some(ComposerReplyDraft {
        ask_before_replacing: true,
        ..record(ReplyDraftState::Ready)
    }));
    let composer = Composer::open(&core);
    composer.draft.draft("Say yes".to_owned());
    settle();
    assert!(composer.question(), "asked before replacing");

    composer.follow(&core, None);
    composer.assert_nothing_drawn();
    assert_eq!(
        core.sent(),
        Vec::<String>::new(),
        "no draft started, no answer sent"
    );

    // A draft that arrived after the person wrote is asked about, and goes the same way.
    composer.follow(
        &core,
        Some(ComposerReplyDraft {
            replace_arrived: true,
            ..record(ReplyDraftState::Drafting)
        }),
    );
    assert!(composer.question(), "asked about the draft that arrived");
    composer.follow(&core, None);
    composer.assert_nothing_drawn();
    assert_eq!(core.sent(), Vec::<String>::new());
    composer.window.destroy();
}
