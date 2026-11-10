// The question a sign-in asks before the browser opens when it would not reach the account's
// calendar or contacts (docs/accounts.md rule 18): keep them on the password, stop using them
// here, or cancel. An `alert`, not a `confirmationDialog`: iPadOS presents the latter as a popover,
// which drops the `.cancel` button, and Cancel is one of the three answers.

import MailcalBindings
import SwiftUI

/// One question on screen, answered exactly once.
@MainActor
final class UnreachedQuestion {
    let set: UnreachedSet
    private var continuation: CheckedContinuation<UnreachedAnswer, Never>?

    private init(set: UnreachedSet, continuation: CheckedContinuation<UnreachedAnswer, Never>) {
        self.set = set
        self.continuation = continuation
    }

    /// Puts the question on screen through `present`, and waits for its answer.
    static func ask(
        _ set: UnreachedSet, present: @MainActor (UnreachedQuestion) -> Void
    ) async -> UnreachedAnswer {
        await withCheckedContinuation { continuation in
            present(UnreachedQuestion(set: set, continuation: continuation))
        }
    }

    /// The first answer stands; a later one, such as the dismissal after a button, is ignored.
    func answer(_ answer: UnreachedAnswer) {
        continuation?.resume(returning: answer)
        continuation = nil
    }
}

private struct UnreachedUsesAlert: ViewModifier {
    @Binding var question: UnreachedQuestion?

    func body(content: Content) -> some View {
        content
            .alert(
                question.map { SeparateServerLabels.unreachedTitle($0.set) } ?? "",
                isPresented: Binding(
                    get: { question != nil },
                    set: { shown in if !shown { dismissed() } }
                ),
                presenting: question
            ) { question in
                Button(L10n.signin_unreached_keep()) { question.answer(.keep) }
                    .keyboardShortcut(.defaultAction)
                // Destructive: it deletes what the device holds of those uses.
                Button(L10n.signin_unreached_stop(), role: .destructive) { question.answer(.stop) }
                Button(L10n.action_cancel(), role: .cancel) { question.answer(.cancel) }
            } message: { _ in
                Text(L10n.signin_unreached_body())
            }
            // A view that leaves while asking must not leave the sign-in waiting forever.
            .onDisappear { question?.answer(.cancel) }
    }

    /// The alert went away. A button's own answer may arrive in the same event, so the cancel
    /// waits a turn and is ignored when it does.
    private func dismissed() {
        let pending = question
        question = nil
        Task { @MainActor in
            await Task.yield()
            pending?.answer(.cancel)
        }
    }
}

/// The question a sign-in the core holds asks (`BrowserSignInStage.asking`), drawn while the core
/// asks it and answered once for its attempt.
private struct SignInQuestionAlert: ViewModifier {
    let question: SignInQuestion?
    let answer: @MainActor (_ attempt: UInt64, _ answer: UnreachedAnswer) -> Void

    /// The attempt whose question was answered.
    @State private var answered: UInt64?
    /// The attempt whose alert went away, so it is not drawn again while its answer is on its way.
    @State private var hidden: UInt64?

    private var shown: SignInQuestion? {
        question.flatMap { $0.attempt == answered || $0.attempt == hidden ? nil : $0 }
    }

    func body(content: Content) -> some View {
        content
            .alert(
                shown.map { SeparateServerLabels.unreachedTitle($0.set) } ?? "",
                isPresented: Binding(
                    get: { shown != nil },
                    set: { visible in if !visible, let shown { dismissed(shown.attempt) } }
                ),
                presenting: shown
            ) { question in
                Button(L10n.signin_unreached_keep()) { give(question.attempt, .keep) }
                    .keyboardShortcut(.defaultAction)
                Button(L10n.signin_unreached_stop(), role: .destructive) {
                    give(question.attempt, .stop)
                }
                Button(L10n.action_cancel(), role: .cancel) { give(question.attempt, .cancel) }
            } message: { _ in
                Text(L10n.signin_unreached_body())
            }
            // A view that leaves while asking cancels the sign-in rather than leave it waiting.
            .onDisappear {
                if let question { give(question.attempt, .cancel) }
            }
    }

    /// The first answer for an attempt stands.
    private func give(_ attempt: UInt64, _ given: UnreachedAnswer) {
        guard answered != attempt else { return }
        answered = attempt
        answer(attempt, given)
    }

    /// The alert went away. A button's own answer may arrive in the same event, so the cancel
    /// waits a turn and is ignored when it does.
    private func dismissed(_ attempt: UInt64) {
        hidden = attempt
        Task { @MainActor in
            await Task.yield()
            give(attempt, .cancel)
        }
    }
}

extension View {
    /// Draws `question` while it is set.
    func unreachedUsesAlert(_ question: Binding<UnreachedQuestion?>) -> some View {
        modifier(UnreachedUsesAlert(question: question))
    }

    /// Draws the question the core's sign-in asks, handing `answer` the first answer given for
    /// its attempt.
    func signInQuestion(
        _ question: SignInQuestion?,
        answer: @escaping @MainActor (_ attempt: UInt64, _ answer: UnreachedAnswer) -> Void
    ) -> some View {
        modifier(SignInQuestionAlert(question: question, answer: answer))
    }
}
