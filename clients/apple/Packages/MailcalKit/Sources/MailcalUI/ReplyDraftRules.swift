// What a composer does with Draft a reply's record that is not drawing (docs/ai.md rules 17 to
// 24): the core verbs it goes through, what the composer keeps while it draws it, the words for a
// suggestion, an item and Send's question, and where a remedy goes. The record decides; these only
// carry and word it, as plain types the unit suite reads without a screen.

import Foundation
import MailcalBindings

/// The core verbs a composer's Draft a reply goes through. Closures over the model, so a preview
/// or a test hands its own.
struct ComposerReplyDrafts {
    /// The record for a composition sending from a From, `nil` while not offered or not a reply.
    let read: (_ composition: String, _ from: String?) -> ComposerReplyDraft?
    /// A change, applied before it returns.
    let change: (ReplyDraftIntent) -> Void
    /// A message the editor posted on its host channel.
    let report: (_ composition: String, _ message: String) -> Void
    /// Opens what a remedy names that the composer cannot take in place; the draft stays.
    let remedy: (AiRemedy) -> Void
    /// Moves on every `Surface::ReplyDraft` and `Surface::WritingStyle`, which name no
    /// composition, so a composer watches it and reads its own record.
    let version: Int
}

/// What a composer keeps while it draws Draft a reply: the record it last read, and which of its
/// questions is up. None of it decides anything the record says.
struct ReplyDraftUI {
    var record: ComposerReplyDraft?
    /// The instruction's input is open.
    var inputShown = false
    /// The instruction as typed in the input.
    var line = ""
    /// Draft was pressed while the person's text is above the signature and the quote.
    var askingReplace = false
    /// Send was pressed while the checklist's question is due.
    var askingSend = false
}

/// Where a remedy goes from a composer.
enum ComposerRemedyRoute: Equatable {
    /// Try again: the instruction's input, in place.
    case input
    /// Somewhere the composer cannot take it: the draft stays and the app opens it.
    case elsewhere
}

extension ComposerRemedyRoute {
    init(_ remedy: AiRemedy) {
        self = remedy == .tryAgain ? .input : .elsewhere
    }
}

/// Where Settings opens for a remedy a composer hands on, or `nil` for one that opens no page
/// (the Allodia sign-in opens over the composer).
func settingsCategory(for remedy: AiRemedy) -> SettingsCategory? {
    switch remedy {
    case .openAiEndpoint: .advanced
    case .learnStyle, .chooseStyle, .chooseAccount, .chooseRange: .writingStyle
    case .signInToAllodia, .tryAgain: nil
    }
}

enum ReplyDraftWords {
    /// A suggestion under the instruction's line, which fills it.
    static func suggestion(_ suggestion: ReplyDraftSuggestion) -> String {
        switch suggestion {
        case .yes: L10n.composer_draft_chip_yes()
        case .no: L10n.composer_draft_chip_no()
        case .moreInformation: L10n.composer_draft_chip_more_info()
        case .holding: L10n.composer_draft_chip_later()
        }
    }

    /// A checklist item: a gap named as the reply carries it, the rest as the core wrote them.
    static func item(_ item: ReplyDraftItem) -> String {
        item.kind == .fillIn ? L10n.composer_task_fill_in(placeholder: item.text) : item.text
    }

    /// Send's question, naming how many items are open.
    static func sendQuestion(_ question: ReplyDraftSendQuestion) -> String {
        L10n.composer_send_open_message(count: Int(question.open))
    }

    /// The catalog locale the app is read in, which the summary and the checklist are written in.
    static var language: String {
        L10n.appLocale.language.languageCode?.identifier ?? "en"
    }
}
