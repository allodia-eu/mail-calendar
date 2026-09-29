// Writing style, drafted replies and the own AI endpoint on the model (docs/ai.md). The library and
// the assignments are the core's; `model.writingStyles` mirrors the snapshot and refreshes on every
// `Surface::WritingStyle` signal, and these dispatch the setters.
//
// Four of the calls block, three on the AI endpoint and one on reading the Sent folder, so they run
// off the main actor the way the Allodia account sync does, and their answers come back here.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// The snapshot before the core has answered: not offered and no route, so no Writing style
    /// category and no own endpoint.
    static let noWritingStyles = WritingStyleSnapshot(
        offered: false, route: nil, refused: nil, styles: [], accounts: [], learning: nil, balance: nil
    )

    /// One style in full, for the reveal.
    func writingStyleDetail(_ id: String) -> WritingStyleDetail? {
        app?.writingStyleDetail(id: id)
    }

    /// Saves the reveal's name and notes.
    func saveWritingStyle(_ id: String, name: String, notes: String) {
        _ = app?.renameWritingStyle(id: id, name: name)
        _ = app?.updateWritingStyleNotes(id: id, notes: notes)
    }

    /// Forgets a style, clearing it from every account that drafted in it.
    func deleteWritingStyle(_ id: String) {
        _ = app?.deleteWritingStyle(id: id)
    }

    /// Assigns (or clears, with `nil`) the style an account drafts in.
    func setAccountWritingStyle(_ account: String, _ style: String?) {
        app?.setAccountWritingStyle(account: account, style: style)
    }

    /// The style `account` drafts in, when it has one.
    func resolveWritingStyle(_ account: String) -> String? {
        app?.resolveWritingStyle(account: account)
    }

    /// What learning from `account` in `range` would read and send. Reads the Sent folder.
    func sentCorpusReport(
        _ account: String, _ range: LearnRangeChoice
    ) async -> Result<CorpusReport, WritingStyleFailure> {
        guard let app else { return .failure(.Unavailable) }
        let bounds = range.bounds(calendar: .current)
        return await blocking {
            try app.sentCorpusReport(accountId: account, since: bounds.since, until: bounds.until)
        }
    }

    /// Learns a style from `account` in `range`. Progress arrives on `writingStyles.learning`.
    func learnWritingStyle(
        _ account: String, _ range: LearnRangeChoice
    ) async -> Result<LearnReport, WritingStyleFailure> {
        guard let app else { return .failure(.Unavailable) }
        let bounds = range.bounds(calendar: .current)
        let name = L10n.writing_style_default_name()
        // The catalog locale the app is shown in, which is what the description is written in.
        let language = L10n.appLocale.identifier
        return await blocking {
            try app.learnWritingStyle(
                accountId: account, since: bounds.since, until: bounds.until,
                name: name, uiLanguage: language
            )
        }
    }

    /// Stops a learning run before its next request.
    func cancelWritingStyleLearning() {
        app?.cancelWritingStyleLearning()
    }

    /// Drafts a reply to `account`/`key` for the open composer. Nothing is sent.
    func draftReply(
        _ account: String, _ key: String, from: String?, intent: String?
    ) async -> Result<DraftReply, WritingStyleFailure> {
        guard let app else { return .failure(.Unavailable) }
        // The summary and the checklist are written in the language the app is shown in.
        let uiLanguage = L10n.appLocale.identifier
        return await blocking {
            try app.draftReply(
                accountId: account, key: key, from: from, style: nil, intent: intent, language: nil,
                uiLanguage: uiLanguage
            )
        }
    }

    /// Whether feedback on a draft is offered: only while signed in to an Allodia account.
    func aiFeedbackAvailable() -> Bool {
        app?.aiFeedbackAvailable() ?? false
    }

    /// Keeps a rating of a draft this session issued, in the core's outbox; quick and local.
    func rateDraft(_ draftId: String, _ rating: DraftRating, includeContent: Bool) -> Bool {
        app?.rateDraft(draftId: draftId, rating: rating, includeContent: includeContent) ?? false
    }

    /// Asks the relay for the balance. A failure changes nothing: the stored line stays.
    func refreshAiBalance() async {
        guard let app, writingStyles.route == .relay else { return }
        _ = await blocking { try app.refreshAiBalance() }
    }

    /// The own endpoint as stored, the key never included.
    func ownAiEndpoint() -> OwnAiEndpoint? {
        app?.ownAiEndpoint()
    }

    /// Saves the own endpoint, or says why it was not saved.
    func setOwnAiEndpoint(
        baseUrl: String, key: String?, model: String, declared: JurisdictionClass?
    ) -> OwnEndpointError? {
        guard let app else { return nil }
        do {
            try app.setOwnAiEndpoint(baseUrl: baseUrl, model: model, declared: declared, key: key)
            return nil
        } catch let error as OwnEndpointError {
            return error
        } catch {
            return .Keystore("\(error)")
        }
    }

    /// Removes the own endpoint and its key. The settings go even when the key could not.
    func clearOwnAiEndpoint() -> OwnEndpointError? {
        do {
            try app?.clearOwnAiEndpoint()
            return nil
        } catch let error as OwnEndpointError {
            return error
        } catch {
            return .Keystore("\(error)")
        }
    }

    /// Runs a blocking core call off the main actor and hands back its typed failure. Anything
    /// else it throws is a failure the core did not name, which reads as an unreadable answer.
    private func blocking<Value: Sendable>(
        _ call: @escaping @Sendable () throws -> Value
    ) async -> Result<Value, WritingStyleFailure> {
        await Task.detached(priority: .userInitiated) {
            do {
                return .success(try call())
            } catch let failure as WritingStyleFailure {
                return .failure(failure)
            } catch {
                return .failure(.Malformed)
            }
        }.value
    }
}
