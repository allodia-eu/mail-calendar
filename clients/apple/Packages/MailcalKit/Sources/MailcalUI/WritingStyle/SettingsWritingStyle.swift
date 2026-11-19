// Settings → Writing style (docs/ai.md rules 8 to 10), drawn from the core's `writing_style_page`:
// the library, Learn or the run in its place, and each account's style. A row opens the reveal the
// first time and the detail after that, as the record says; Learn opens the learning flow. Every
// control is the core's `ControlState`, drawn through the control components.

import MailcalBindings
import SwiftUI

struct SettingsWritingStyle: View {
    var model: MailboxModel

    /// The style whose reveal or detail is open.
    @State private var opened: OpenedStyle?
    @State private var session = WritingStylePageSession(page: nil)
    /// Why the last change to an account's style did not take.
    @State private var problem: String?

    var body: some View {
        if let page = model.writingStyle.page {
            VStack(alignment: .leading, spacing: 20) {
                // No heading: the category's own title is just above it on every platform.
                SettingsGroup(heading: "", description: L10n.writing_style_intro()) {
                    VStack(alignment: .leading, spacing: 14) {
                        library(page)
                        learnArea(page)
                    }
                }
                if !page.accounts.isEmpty {
                    accountsGroup(page)
                }
            }
            .onAppear { session = WritingStylePageSession(page: page) }
            .onChange(of: page.outcome?.serial) { _, _ in
                if let said = session.take(page.outcome) {
                    problem = said
                    AccessibilityNotification.Announcement(said).post()
                }
            }
            .sheet(item: $opened) { style in
                WritingStyleShownSheet(model: model, style: style) { opened = nil }
            }
            .sheet(isPresented: learning) {
                WritingStyleLearningSheet(model: model)
            }
        }
    }

    /// The flow's sheet is up exactly while the core draws the flow; a swipe down closes it.
    private var learning: Binding<Bool> {
        Binding(
            get: { model.writingStyle.learning != nil },
            set: { shown in if !shown { model.learnStyle(.close) } }
        )
    }

    // MARK: The library

    @ViewBuilder
    private func library(_ page: WritingStylePage) -> some View {
        if page.styles.isEmpty {
            Text(L10n.writing_style_empty())
                .foregroundStyle(.secondary)
        } else {
            VStack(alignment: .leading, spacing: 0) {
                ForEach(page.styles, id: \.id) { style in
                    ControlNavRow(
                        title: style.name, subtitle: subtitle(style.facts),
                        enabled: style.open.enabled
                    ) {
                        opened = OpenedStyle(id: style.id, opens: style.opens)
                    }
                    .accessibilityIdentifier("writingStyle.row.\(style.id)")
                    if style.id != page.styles.last?.id { Divider() }
                }
            }
        }
    }

    private func subtitle(_ facts: WritingStyleFacts) -> String {
        let learned = WritingStyleWords.facts(facts)
        guard !facts.languages.isEmpty else { return learned }
        return learned + "\n" + L10n.writing_style_languages(
            languages: WritingStyleWords.languages(facts.languages))
    }

    // MARK: Learn, or the run in its place

    @ViewBuilder
    private func learnArea(_ page: WritingStylePage) -> some View {
        switch page.run {
        case let .learning(done, total, stop):
            VStack(alignment: .leading, spacing: 8) {
                if total > 0 {
                    ProgressView(value: Double(done), total: Double(total))
                } else {
                    ProgressView().progressViewStyle(.linear)
                }
                HStack(alignment: .firstTextBaseline) {
                    Text(WritingStyleWords.progress(done: done, total: total))
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 8)
                    ControlButton(title: L10n.learn_stop(), control: stop) {
                        model.learnStyle(.stop)
                    }
                    .accessibilityIdentifier("writingStyle.stop")
                }
            }
            .accessibilityElement(children: .contain)
        case let .failed(failure, _):
            VStack(alignment: .leading, spacing: 8) {
                Label(
                    L10n.writing_style_run_failed() + " " + AiWords.failure(failure),
                    systemImage: "exclamationmark.triangle"
                )
                .font(.callout)
                .foregroundStyle(.orange)
                .fixedSize(horizontal: false, vertical: true)
                learnButton(page)
            }
        case nil:
            VStack(alignment: .leading, spacing: 8) {
                learnButton(page)
                if let block = page.learnBlocked {
                    blockedLine(block)
                }
            }
        }
        if let problem {
            Text(problem)
                .font(.callout)
                .foregroundStyle(.red)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func learnButton(_ page: WritingStylePage) -> some View {
        ControlButton(
            title: L10n.writing_style_learn(), systemImage: "text.badge.plus", control: page.learn
        ) {
            problem = nil
            model.learnStyle(.open)
        }
        .buttonStyle(.borderedProminent)
        .accessibilityIdentifier("writingStyle.learn")
    }

    /// Why Learn is held, with the one thing that changes it.
    private func blockedLine(_ block: WritingStyleLearnBlock) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(WritingStyleWords.blocked(block))
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            if case let .cannotSend(_, remedy?) = block {
                ControlButton(title: AiWords.remedy(remedy), control: .free()) {
                    model.openRemedy(remedy)
                }
                .buttonStyle(.bordered)
            }
        }
    }

    // MARK: Each account's style

    private func accountsGroup(_ page: WritingStylePage) -> some View {
        SettingsGroup(heading: L10n.writing_style_accounts_heading(), description: "") {
            VStack(alignment: .leading, spacing: 14) {
                ForEach(page.accounts, id: \.accountId) { slot in
                    ControlPicker(
                        title: slot.email,
                        labels: [L10n.writing_style_none()] + page.styles.map(\.name),
                        selected: page.styles.firstIndex { $0.id == slot.selected }.map { $0 + 1 }
                            ?? 0,
                        control: slot.picker
                    ) { index in
                        problem = nil
                        model.changeWritingStyle(.assign(
                            account: slot.accountId,
                            style: index == 0 ? nil : page.styles[index - 1].id))
                    }
                }
            }
        }
    }
}

/// A style the page opened, and which of the two it opens.
struct OpenedStyle: Identifiable, Equatable {
    let id: String
    let opens: WritingStyleOpens
}

/// The reveal or the detail of one style, in a sheet of its own.
private struct WritingStyleShownSheet: View {
    var model: MailboxModel
    let style: OpenedStyle
    let close: () -> Void

    var body: some View {
        switch style.opens {
        case .reveal:
            WritingStyleRevealView(model: model, styleId: style.id, close: close)
        case .detail:
            WritingStyleDetailView(model: model, styleId: style.id, close: close)
        }
    }
}
