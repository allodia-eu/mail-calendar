// Settings → Writing style on the model (docs/ai.md rules 8 to 16): the page and the learning flow
// as the core draws them, read again on `Surface::WritingStyle` (and the page on
// `Surface::Settings`), and the intents the screens send. A reveal and a detail are read by the
// sheet that shows them, again whenever `version` moves.

import MailcalBindings

/// What the model holds of the feature's Settings surfaces.
struct WritingStyleState {
    /// The page, or `nil` while the feature is not offered, which takes the category away.
    var page: WritingStylePage?
    /// The learning flow while a host draws it.
    var learning: WritingStyleLearning?
    /// Bumped on every `Surface::WritingStyle`, so an open reveal or detail reads its record again.
    var version = 0
    /// Bumped on every `Surface::ReplyDraft`, which names no composition: each open composer reads
    /// its own Draft a reply again.
    var replyDraftVersion = 0
    /// Bumped when a remedy asks for the own endpoint's form, which the Settings shells answer by
    /// opening Advanced.
    var endpointRequest = 0
}

extension MailboxModel {
    /// Reads the page and the flow again.
    func reloadWritingStyle() {
        writingStyle.page = app?.writingStylePage()
        writingStyle.learning = app?.writingStyleLearning()
        writingStyle.version &+= 1
    }

    /// The reveal of `style`, or `nil` when it names none.
    func writingStyleReveal(_ style: String) -> WritingStyleReveal? {
        app?.writingStyleReveal(style: style)
    }

    /// The detail of `style`, or `nil` when it names none.
    func writingStyleDetail(_ style: String) -> WritingStyleDetail? {
        app?.writingStyleDetail(style: style)
    }

    /// Moves the learning flow. Returns at once.
    func learnStyle(_ intent: StyleLearningIntent) {
        changeWritingStyle(.learning(intent: intent))
    }

    /// Does what `remedy` names. One in the flow or on the page's run is the flow's `remedy` intent,
    /// which moves the flow or decides whether it stays open, and then opens what it names; one
    /// beside a held Learn only opens it.
    func openRemedy(_ remedy: AiRemedy, inFlow: Bool) {
        let route = WritingStyleRemedyRoute(remedy)
        if inFlow { learnStyle(.remedy) }
        switch route {
        case .allodiaSignIn: signInToAllodia()
        case .endpointForm: writingStyle.endpointRequest &+= 1
        case .learn: learnStyle(.open)
        case .flow, .accountStyles: break
        }
    }

    /// What Draft a reply draws in `composition`'s composer sending from `from`, or `nil` while the
    /// assistant is not offered or the composition is not a reply. Read from memory.
    func composerReplyDraft(_ composition: String, from: String?) -> ComposerReplyDraft? {
        app?.composerReplyDraft(composition: composition, from: from)
    }

    /// A change to a composer's Draft a reply, applied before it returns.
    func changeReplyDraft(_ intent: ReplyDraftIntent) {
        app?.dispatch(intent: .ai(intent: .replyDraft(intent: intent)))
    }

    /// A message `composition`'s editor posted on its host channel, every one of them: the core
    /// takes its reply-draft report and leaves the rest.
    func reportReplyDraft(_ composition: String, _ message: String) {
        _ = app?.composerReplyDraftReport(composition: composition, message: message)
    }

    /// Both signals a composer's Draft a reply follows, as one value that moves with either.
    var replyDraftSignals: Int {
        writingStyle.version &+ writingStyle.replyDraftVersion
    }

    /// A change on the page, the reveal or the detail. Returns at once; what a library change came
    /// to is the page's `outcome`.
    func changeWritingStyle(_ intent: WritingStyleIntent) {
        app?.dispatch(intent: .ai(intent: .writingStyle(intent: intent)))
    }
}
