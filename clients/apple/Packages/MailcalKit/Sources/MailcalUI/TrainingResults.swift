// The right half of Develop → Compare drafts… in a macOS debug build (docs/ai.md, "Training mode
// (debug builds)"): the summary per model and variant, then every result grouped by the message it
// answers, each rated with the controls feedback uses. Plain English, because nothing here ships.

#if DEBUG && os(macOS)
import MailcalBindings
import SwiftUI

/// The summary, then every message of the run with its results, in the order drafted.
struct TrainingResults: View {
    @Bindable var bench: TrainingBench

    var body: some View {
        if bench.results.isEmpty {
            Text(bench.running ? "Drafting…" : "Run to compare drafts here.")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 12) {
                    TrainingSummaryCard(lines: bench.summary)
                    ForEach($bench.groups) { $group in
                        if !group.results.isEmpty {
                            Text(title(group.message))
                                .font(.title3.weight(.semibold))
                                .lineLimit(1)
                                .padding(.top, 8)
                            ForEach(group.results.indices, id: \.self) { index in
                                TrainingResultCard(
                                    result: group.results[index],
                                    feedback: $group.ratings[index],
                                    keep: { rating, _ in bench.rate(group.id, index, rating) }
                                )
                            }
                        }
                    }
                }
                .padding(16)
            }
        }
    }

    private func title(_ message: TrainingMessage) -> String {
        let parts = [message.from, message.subject].filter { !$0.isEmpty }
        return parts.isEmpty ? message.key : parts.joined(separator: " · ")
    }
}

/// Per model and variant: how many drafts came back and how many failed, why, the median time
/// and the tokens.
private struct TrainingSummaryCard: View {
    let lines: [TrainingSummary]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Summary").font(.headline)
            ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                VStack(alignment: .leading, spacing: 2) {
                    Text("\(line.model) · \(line.variant ?? "default")")
                        .font(.callout.weight(.semibold))
                    Text(facts(line))
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                    ForEach(line.failed, id: \.failure) { failed in
                        Text("\(failed.count) failed: \(failed.failure)")
                            .font(.caption)
                            .foregroundStyle(.red)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(.fill.tertiary, in: RoundedRectangle(cornerRadius: 10))
    }

    private func facts(_ line: TrainingSummary) -> String {
        let failed = line.failed.reduce(0) { $0 + Int($1.count) }
        var parts = ["\(line.succeeded) drafted", "\(failed) failed"]
        if let median = line.medianElapsedMs {
            parts.append(String(format: "median %.1f s", Double(median) / 1000))
        }
        parts.append("\(line.promptTokens) tokens in, \(line.completionTokens) out")
        return parts.joined(separator: " · ")
    }
}

private struct TrainingResultCard: View {
    let result: TrainingResult
    @Binding var feedback: DraftFeedback
    let keep: (DraftRating, Bool) -> Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline) {
                Text("\(result.model) · \(result.variant ?? "default")").font(.headline)
                Spacer()
                Text(facts).font(.caption.monospacedDigit()).foregroundStyle(.secondary)
            }
            if let failure = result.failure {
                Text(failure).foregroundStyle(.red)
            } else {
                if !result.summary.isEmpty {
                    Text(result.summary).font(.callout).foregroundStyle(.secondary)
                }
                ForEach(Array(result.tasks.enumerated()), id: \.offset) { _, task in
                    Label(task.text, systemImage: icon(task.kind)).font(.callout)
                }
                Text(result.reply)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
                    .background(.fill.quaternary, in: RoundedRectangle(cornerRadius: 6))
                DraftRatingControls(feedback: $feedback, isFeedback: false, keep: keep)
            }
        }
        .padding(12)
        .background(.fill.tertiary, in: RoundedRectangle(cornerRadius: 10))
    }

    private var facts: String {
        let seconds = String(format: "%.1f s", Double(result.elapsedMs) / 1000)
        guard let usage = result.usage else { return seconds }
        return "\(seconds) · \(usage.promptTokens) in, \(usage.completionTokens) out"
    }

    private func icon(_ kind: DraftTaskKind) -> String {
        switch kind {
        case .fillIn: "character.cursor.ibeam"
        case .attach: "paperclip"
        case .do: "checklist"
        }
    }
}
#endif
