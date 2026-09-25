// The "Learn my writing style" sheet (docs/ai.md, "Learning"): an account when there is a choice,
// the range, what the device found in it with the consent beneath, then the run, one page each in
// the frame the reveal uses.
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

    private let steps: [LearnStep]
    @State private var flow: LearnWritingStyleFlow
    @State private var pager: WizardPager
    @State private var usesUntil = false
    @State private var untilDay = Date()
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    init(model: MailboxModel, finish: @escaping (String?) -> Void) {
        self.model = model
        self.finish = finish
        let accounts = model.writingStyles.accounts.map(\.accountId)
        steps = LearnWritingStyleFlow.steps(accounts)
        _flow = State(initialValue: LearnWritingStyleFlow(accounts: accounts))
        _pager = State(initialValue: WizardPager(count: steps.count))
    }

    private var accounts: [AccountWritingStyleRow] { model.writingStyles.accounts }
    private var step: LearnStep { steps[pager.index] }

    var body: some View {
        WizardSheet(
            pager: $pager,
            cancel: step == .progress
                ? nil
                : WizardAction(title: L10n.action_cancel(), role: .cancel, prominent: false) { finish(nil) },
            showsBack: step != .progress && !pager.isFirst,
            primary: primary
        ) { index in
            page(steps[index])
        }
        .interactiveDismissDisabled(!flow.dismissible)
        .onChange(of: usesUntil) { _, _ in syncRange() }
        .onChange(of: untilDay) { _, _ in syncRange() }
        .task(id: reportKey) { await readReport() }
    }

    private var primary: WizardPrimary {
        switch step {
        case .account, .range:
            return .next(enabled: flow.account != nil)
        case .consent:
            // Nothing usable says so and offers no button.
            guard flow.canLearn else { return .none }
            return .action(WizardAction(title: L10n.learn_consent_confirm(), action: learn))
        case .progress:
            switch flow.phase {
            case .learning:
                return .action(WizardAction(title: L10n.learn_stop(), role: .cancel, prominent: false) {
                    model.cancelWritingStyleLearning()
                })
            case .failed:
                return .action(WizardAction(title: L10n.action_close()) { finish(nil) })
            case .choosing, .learned:
                return .none
            }
        }
    }

    @ViewBuilder
    private func page(_ step: LearnStep) -> some View {
        switch step {
        case .account:
            WizardPage(title: L10n.learn_account_title()) {
                ChoiceList(
                    title: L10n.learn_account_title(),
                    selection: $flow.account,
                    options: accounts.map { (Optional($0.accountId), $0.email) }
                )
            }
        case .range:
            WizardPage(title: L10n.learn_range_title()) { range }
        case .consent:
            WizardPage(title: L10n.writing_style_learn()) { report }
        case .progress:
            if case let .failed(failure) = flow.phase {
                WizardPage(title: L10n.learn_failed_title()) { failureText(failure) }
            } else {
                progress
            }
        }
    }

    // MARK: Range, report, consent

    @ViewBuilder
    private var range: some View {
        ChoiceList(title: L10n.learn_range_title(), selection: $usesUntil, options: [
            (false, L10n.learn_range_all()),
            (true, L10n.learn_range_until()),
        ])
        if usesUntil {
            Text(L10n.learn_range_until_hint())
                .font(WizardFont.caption)
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
            .font(WizardFont.text)
            .fixedSize(horizontal: false, vertical: true)
            if report.usable == 0 {
                Text(L10n.learn_report_nothing())
                    .font(WizardFont.text)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                VStack(alignment: .leading, spacing: 6) {
                    Text(L10n.learn_consent_title())
                        .font(WizardFont.label)
                        .foregroundStyle(.secondary)
                        .accessibilityAddTraits(.isHeader)
                    Text(consentText)
                        .font(WizardFont.text)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.top, 6)
            }
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

    /// The bar and the count, alone in the middle of the page. A finished run shows the bar full
    /// while the sheet hands over to the reveal.
    private var progress: some View {
        VStack(spacing: 12) {
            if case .learned = flow.phase {
                ProgressView(value: 1).frame(maxWidth: 320)
            } else if let learning = model.writingStyles.learning, learning.stage == .learning {
                ProgressView(value: Double(learning.done), total: Double(max(learning.total, 1)))
                    .frame(maxWidth: 320)
                    .animation(reduceMotion ? nil : .easeOut(duration: 0.4), value: learning.done)
                Text(L10n.learn_progress(done: String(learning.done), total: String(learning.total)))
                    .font(WizardFont.text)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
            } else {
                reading
            }
        }
        .padding(WizardMetrics.gutter)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var reading: some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text(L10n.learn_reading()).font(WizardFont.text).foregroundStyle(.secondary)
        }
    }

    private func failureText(_ failure: WritingStyleFailure) -> some View {
        Text(writingStyleFailureText(failure, route: model.writingStyles.route))
            .font(WizardFont.text)
            .foregroundStyle(failure == .Cancelled ? Color.secondary : Color.red)
            .fixedSize(horizontal: false, vertical: true)
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
        withAnimation(reduceMotion ? nil : WizardMotion.page) { pager.go(to: steps.count - 1) }
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
