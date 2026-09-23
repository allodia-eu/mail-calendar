// The "Learn my writing style" sheet (docs/ai.md, "Learning"): an account when there is a choice,
// the range, what the device found in it, and the consent, all on one sheet, then the run.
//
// The report is read again whenever the account or the range changes, so what the consent says is
// always about the mail the Learn button would send. Pressing Learn is the consent; there is no
// other prompt. `LearnWritingStyleFlow` decides what each answer means.

import Foundation
import MailcalBindings
import SwiftUI

struct LearnWritingStyleSheet: View {
    var model: MailboxModel
    /// Called when the sheet is done: with the new style's id after a run that learned one, so the
    /// caller opens it, and with `nil` otherwise.
    let finish: (String?) -> Void

    @State private var flow: LearnWritingStyleFlow
    @State private var usesUntil = false
    @State private var untilDay = Date()

    init(model: MailboxModel, finish: @escaping (String?) -> Void) {
        self.model = model
        self.finish = finish
        _flow = State(initialValue: LearnWritingStyleFlow(
            accounts: model.writingStyles.accounts.map(\.accountId)
        ))
    }

    private var accounts: [AccountWritingStyleRow] { model.writingStyles.accounts }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.writing_style_learn()).font(.headline)
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    switch flow.phase {
                    case .choosing: choosing
                    case .learning: progress
                    case let .failed(failure): failed(failure)
                    case .learned: EmptyView()
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            buttons
        }
        .padding(16)
        #if os(macOS)
        .frame(minWidth: 480, minHeight: 440)
        #else
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        #endif
        .interactiveDismissDisabled(!flow.dismissible)
        .task(id: reportKey) { await readReport() }
    }

    // MARK: Account, range, report, consent

    @ViewBuilder
    private var choosing: some View {
        if LearnWritingStyleFlow.asksForAccount(accounts.map(\.accountId)) {
            VStack(alignment: .leading, spacing: 6) {
                Text(L10n.learn_account_title()).font(.subheadline).bold()
                ChoiceList(
                    title: L10n.learn_account_title(),
                    selection: $flow.account,
                    options: accounts.map { (Optional($0.accountId), $0.email) }
                )
            }
        }
        VStack(alignment: .leading, spacing: 6) {
            Text(L10n.learn_range_title()).font(.subheadline).bold()
            ChoiceList(title: L10n.learn_range_title(), selection: $usesUntil, options: [
                (false, L10n.learn_range_all()),
                (true, L10n.learn_range_until()),
            ])
            if usesUntil {
                Text(L10n.learn_range_until_hint())
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                DatePicker(
                    L10n.learn_range_until_label(),
                    selection: $untilDay,
                    in: ...Date(),
                    displayedComponents: .date
                )
                .environment(\.locale, L10n.appLocale)
            }
        }
        .onChange(of: usesUntil) { _, _ in syncRange() }
        .onChange(of: untilDay) { _, _ in syncRange() }
        Divider()
        report
    }

    @ViewBuilder
    private var report: some View {
        switch flow.report {
        case .reading:
            reading
        case let .failed(failure):
            failureText(failure)
        case let .ready(report):
            VStack(alignment: .leading, spacing: 4) {
                Text(L10n.learn_report_found(count: Int(report.found)))
                Text(L10n.learn_report_usable(count: Int(report.usable)))
                if !report.languages.isEmpty {
                    Text(L10n.learn_report_languages(languages: writingStyleLanguages(
                        report.languages.map(\.language), locale: L10n.appLocale
                    )))
                }
                if report.undetected > 0 {
                    Text(L10n.learn_report_undetected(count: Int(report.undetected)))
                }
                if let horizon = report.horizon {
                    Text(L10n.learn_report_horizon(date: writingStyleDate(
                        horizon, zone: displayZone, locale: L10n.appLocale
                    )))
                    .foregroundStyle(.secondary)
                }
            }
            .font(.callout)
            if report.usable == 0 {
                Text(L10n.learn_report_nothing())
                    .font(.callout)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                consent
            }
        }
    }

    private var consent: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(L10n.learn_consent_title()).font(.subheadline).bold()
            Text(consentText)
                .font(.callout)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    /// Where the words go, named for the route they take.
    private var consentText: String {
        if model.writingStyles.route == .relay { return L10n.learn_consent_relay() }
        let host = model.ownAiEndpoint()
            .flatMap { URL(string: $0.baseUrl)?.host() } ?? ""
        return L10n.learn_consent_own(host: host)
    }

    // MARK: The run, and how it ended

    @ViewBuilder
    private var progress: some View {
        if let learning = model.writingStyles.learning, learning.stage == .learning {
            VStack(alignment: .leading, spacing: 8) {
                Text(L10n.learn_progress(done: String(learning.done), total: String(learning.total)))
                    .font(.callout)
                ProgressView(value: Double(learning.done), total: Double(max(learning.total, 1)))
            }
        } else {
            reading
        }
    }

    private var reading: some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text(L10n.learn_reading()).font(.callout).foregroundStyle(.secondary)
        }
    }

    private func failed(_ failure: WritingStyleFailure) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(L10n.learn_failed_title()).font(.subheadline).bold()
            failureText(failure)
        }
    }

    private func failureText(_ failure: WritingStyleFailure) -> some View {
        Text(writingStyleFailureText(failure, route: model.writingStyles.route))
            .font(.callout)
            .foregroundStyle(failure == .Cancelled ? Color.secondary : Color.red)
            .fixedSize(horizontal: false, vertical: true)
    }

    // MARK: Buttons

    @ViewBuilder
    private var buttons: some View {
        HStack {
            Spacer()
            switch flow.phase {
            case .learning:
                Button(L10n.learn_stop(), role: .cancel) { model.cancelWritingStyleLearning() }
            case .failed, .learned:
                Button(L10n.action_close()) { finish(nil) }
                    .keyboardShortcut(.defaultAction)
            case .choosing:
                Button(L10n.action_cancel(), role: .cancel) { finish(nil) }
                if flow.canLearn {
                    Button(L10n.learn_consent_confirm()) { learn() }
                        .buttonStyle(.borderedProminent)
                        .keyboardShortcut(.defaultAction)
                }
            }
        }
    }

    // MARK: Driving the core

    /// What the report depends on; a change reads it again.
    private var reportKey: String {
        "\(flow.account ?? "")|\(flow.range)"
    }

    private func syncRange() {
        flow.range = usesUntil ? .until(untilDay) : .everything
    }

    private func readReport() async {
        guard flow.phase == .choosing, let account = flow.account else { return }
        let generation = flow.readReport()
        let result = await model.sentCorpusReport(account, flow.range)
        flow.reportArrived(result, generation: generation)
    }

    private func learn() {
        guard let account = flow.account, flow.consent() else { return }
        let range = flow.range
        Task {
            flow.learningFinished(await model.learnWritingStyle(account, range))
            if case let .learned(styleId) = flow.phase { finish(styleId) }
        }
    }

    private var displayZone: TimeZone {
        TimeZone(identifier: model.activeZone) ?? .current
    }
}
