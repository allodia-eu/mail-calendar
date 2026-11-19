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

    /// Does what `remedy` names: moves the flow, or closes it and opens what the remedy names.
    func openRemedy(_ remedy: AiRemedy) {
        let route = WritingStyleRemedyRoute(remedy)
        if route == .flow {
            learnStyle(.remedy)
            return
        }
        if writingStyle.learning != nil { learnStyle(.close) }
        switch route {
        case .allodiaSignIn: signInToAllodia()
        case .endpointForm: writingStyle.endpointRequest &+= 1
        case .learn: learnStyle(.open)
        case .flow, .accountStyles: break
        }
    }

    /// A change on the page, the reveal or the detail. Returns at once; what a library change came
    /// to is the page's `outcome`.
    func changeWritingStyle(_ intent: WritingStyleIntent) {
        app?.dispatch(intent: .ai(intent: .writingStyle(intent: intent)))
    }
}
