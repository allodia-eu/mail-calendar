// What each stage of the learning flow shows (docs/ai.md rule 11), drawn from the stage the core
// gives: the accounts, the ranges it offers and how far back the device reaches, what the review
// read and where Learn would send it, the run's progress, and a failure. The flow host around it
// draws the controls.

import MailcalBindings
import SwiftUI

struct WritingStyleLearningStage: View {
    var model: MailboxModel
    let stage: StyleLearningStage

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            switch stage {
            case let .chooseAccount(accounts, selected):
                StyleChoiceList(
                    options: accounts.map { StyleChoice(id: $0.accountId, title: $0.email) },
                    selected: selected
                ) { model.learnStyle(.selectAccount(account: $0)) }
            case let .chooseRange(email, presets, earliest, latest, selected, reach):
                StyleRangeStage(
                    model: model, email: email, presets: presets, earliest: earliest,
                    latest: latest, selected: selected, reach: reach)
            case let .review(email, range, reach, review, sendsTo, cannotSend, keywordKept):
                StyleReviewStage(
                    email: email, range: range, reach: reach, review: review, sendsTo: sendsTo,
                    cannotSend: cannotSend, keywordKept: keywordKept)
            case let .learning(done, total):
                StyleProgress(done: done, total: total)
            case let .failed(failure, _):
                Label(AiWords.failure(failure), systemImage: "exclamationmark.triangle")
                    .font(.body)
                    .foregroundStyle(.primary)
                    .fixedSize(horizontal: false, vertical: true)
            case .learned:
                EmptyView()
            }
        }
    }
}

/// One answer of a choice of one.
struct StyleChoice: Identifiable {
    let id: String
    let title: String
    var note: String?
}

/// A choice of one: a radio group on macOS, a checkmark list on iOS and iPadOS, as each platform's
/// own Settings draws one.
struct StyleChoiceList: View {
    let options: [StyleChoice]
    let selected: String?
    let pick: (String) -> Void

    var body: some View {
        #if os(macOS)
        Picker(
            "",
            selection: Binding(get: { selected }, set: { if let id = $0, id != selected { pick(id) } })
        ) {
            ForEach(options) { option in
                VStack(alignment: .leading, spacing: 2) {
                    Text(option.title)
                    if let note = option.note {
                        Text(note).font(.caption).foregroundStyle(.secondary)
                    }
                }
                .tag(String?.some(option.id))
            }
        }
        .pickerStyle(.radioGroup)
        .labelsHidden()
        #else
        VStack(alignment: .leading, spacing: 0) {
            ForEach(options) { option in
                Button {
                    if option.id != selected { pick(option.id) }
                } label: {
                    HStack(alignment: .firstTextBaseline, spacing: 10) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(option.title).foregroundStyle(.primary)
                            if let note = option.note {
                                Text(note).font(.caption).foregroundStyle(.secondary)
                            }
                        }
                        .multilineTextAlignment(.leading)
                        .fixedSize(horizontal: false, vertical: true)
                        Spacer(minLength: 0)
                        Image(systemName: "checkmark")
                            .foregroundStyle(.tint)
                            .opacity(option.id == selected ? 1 : 0)
                            .accessibilityHidden(true)
                    }
                    .frame(minHeight: 44)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(option.id == selected ? .isSelected : [])
                if option.id != options.last?.id { Divider() }
            }
        }
        // An inset group, as iOS Settings draws a choice of one.
        .padding(.horizontal, 16)
        .background(
            Color(uiColor: .secondarySystemGroupedBackground), in: RoundedRectangle(cornerRadius: 12))
        #endif
    }
}

/// Which sent mail: the core's presets, and a range of the person's own within its bounds.
private struct StyleRangeStage: View {
    var model: MailboxModel
    let email: String
    let presets: [StyleLearnRange]
    let earliest: String?
    let latest: String
    let selected: StyleLearnRange?
    let reach: StyleSentReach

    var body: some View {
        Text(email).font(.callout).foregroundStyle(.secondary)
        StyleChoiceList(options: options, selected: selectedOption) { id in
            if let index = Int(id) { model.learnStyle(.selectRange(range: presets[index])) }
        }
        if case let .between(since, until)? = selected {
            betweenDays(since: since, until: until)
        }
        Text(WritingStyleWords.reach(reach))
            .font(.callout)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
    }

    /// The core's presets, a range of the person's own last.
    private var options: [StyleChoice] {
        presets.enumerated().map { index, preset in
            StyleChoice(
                id: String(index), title: WritingStyleWords.range(preset),
                note: preset.samePreset(as: .untilEndOfLastYear) ? L10n.learn_range_until_hint() : nil)
        }
    }

    private var selectedOption: String? {
        presets.firstIndex { $0.samePreset(as: selected) }.map(String.init)
    }

    /// The two days of a range of the person's own, each bounded by the other and by what the
    /// device holds. A day is the core's `YYYY-MM-DD`, in UTC.
    private func betweenDays(since: String, until: String) -> some View {
        let low = earliest.flatMap(WritingStyleWords.dayDate) ?? .distantPast
        let high = WritingStyleWords.dayDate(latest) ?? .distantFuture
        let from = WritingStyleWords.dayDate(since) ?? low
        let to = WritingStyleWords.dayDate(until) ?? high
        return VStack(alignment: .leading, spacing: 8) {
            DatePicker(
                L10n.learn_range_from(),
                selection: Binding(get: { from }, set: { pickRange(since: $0, until: to) }),
                in: low...max(low, to), displayedComponents: .date)
            DatePicker(
                L10n.learn_range_until_label(),
                selection: Binding(get: { to }, set: { pickRange(since: from, until: $0) }),
                in: min(from, high)...high, displayedComponents: .date)
        }
        .environment(\.timeZone, TimeZone(identifier: "UTC") ?? .gmt)
        .environment(\.locale, L10n.appLocale)
        .padding(.leading, 4)
    }

    private func pickRange(since: Date, until: Date) {
        model.learnStyle(.selectRange(range: .between(
            since: WritingStyleWords.dayText(since), until: WritingStyleWords.dayText(until))))
    }
}

/// What the review read on the device, and what Learn would send where. Nothing has left yet.
private struct StyleReviewStage: View {
    let email: String
    let range: StyleLearnRange
    let reach: StyleSentReach
    let review: StyleLearningReview
    let sendsTo: StyleLearningDestination?
    let cannotSend: AiFailure?
    let keywordKept: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(email).font(.callout).foregroundStyle(.secondary)
            Text(WritingStyleWords.rangeRead(range)).font(.callout).foregroundStyle(.secondary)
        }
        switch review {
        case .reading:
            HStack(spacing: 10) {
                ProgressView().controlSize(.small)
                Text(L10n.learn_reading()).foregroundStyle(.secondary)
            }
        case let .ready(report):
            reportView(report)
        case let .nothingUsable(report):
            reportView(report)
            Label(L10n.learn_report_nothing(), systemImage: "exclamationmark.circle")
                .foregroundStyle(.orange)
                .fixedSize(horizontal: false, vertical: true)
        }
        Text(WritingStyleWords.reach(reach))
            .font(.callout)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
        consent
        if !keywordKept {
            Label(L10n.learn_keyword_not_kept(), systemImage: "info.circle")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func reportView(_ report: StyleLearningReport) -> some View {
        StyleCard {
            VStack(alignment: .leading, spacing: 6) {
                fact(L10n.learn_report_found(count: Int(report.found)))
                fact(L10n.learn_report_usable(count: Int(report.usable)))
                if !report.languages.isEmpty {
                    fact(L10n.learn_report_languages(
                        languages: WritingStyleWords.languages(report.languages.map(\.language))))
                }
                if report.otherLanguages > 0 {
                    fact(L10n.learn_report_undetected(count: Int(report.otherLanguages)))
                }
                if let oldest = report.oldest, let newest = report.newest {
                    fact(L10n.learn_report_dates(
                        oldest: WritingStyleWords.date(oldest),
                        newest: WritingStyleWords.date(newest)))
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func fact(_ text: String) -> some View {
        Text(text).fixedSize(horizontal: false, vertical: true)
    }

    /// What is sent where, or why nothing can be: pressing Learn below it is the consent.
    private var consent: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(L10n.learn_consent_title()).font(.headline).accessibilityAddTraits(.isHeader)
            if let sendsTo {
                Text(WritingStyleWords.consent(sendsTo))
                    .fixedSize(horizontal: false, vertical: true)
            } else if let cannotSend {
                Label(AiWords.failure(cannotSend), systemImage: "exclamationmark.triangle")
                    .foregroundStyle(.orange)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(.top, 4)
    }
}

/// A group on the page: the system's grouped background on iOS, a box on macOS.
struct StyleCard<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        #if os(macOS)
        GroupBox { content }
        #else
        content
            .padding(16)
            .background(
                Color(uiColor: .secondarySystemGroupedBackground),
                in: RoundedRectangle(cornerRadius: 12))
        #endif
    }
}

/// The run's progress: a bar once the run knows its total, a moving one before.
private struct StyleProgress: View {
    let done: UInt32
    let total: UInt32

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if total > 0 {
                ProgressView(value: Double(done), total: Double(total))
            } else {
                ProgressView().progressViewStyle(.linear)
            }
            Text(WritingStyleWords.progress(done: done, total: total))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .accessibilityElement(children: .combine)
    }
}
