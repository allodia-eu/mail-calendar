// Develop → Compare drafts… in a macOS debug build (docs/ai.md, "Training mode (debug builds)"): the
// models and variants on the left, the run's results on the right, each rated with the controls
// feedback uses, and the run exported as JSON. Plain English, because nothing here ships.

#if DEBUG && os(macOS)
import MailcalBindings
import SwiftUI

/// The Develop menu, whose one item opens the window.
public struct TrainingCommands: Commands {
    @Environment(\.openWindow) private var openWindow

    public init() {}

    public var body: some Commands {
        CommandMenu("Develop") {
            Button("Compare drafts…") { openWindow(id: TrainingWindow.id) }
        }
    }
}

public struct TrainingWindow: View {
    public static let id = "training"

    @Bindable var model: MailboxModel
    @State private var bench = TrainingBench()
    /// The variant being read or edited; `nil` for the default instructions.
    @State private var selected: TrainingVariantDraft.ID?

    public init(session: AppSession) {
        model = session.model
    }

    public var body: some View {
        HSplitView {
            setup
                .frame(minWidth: 340, idealWidth: 420, maxWidth: 560)
            TrainingResults(bench: bench)
                .frame(minWidth: 420, maxWidth: .infinity, maxHeight: .infinity)
        }
        .frame(minWidth: 860, minHeight: 560)
    }

    private var setup: some View {
        VStack(alignment: .leading, spacing: 14) {
            section("Message") {
                if let message = model.trainingMessage {
                    Text(message.key == model.reading?.key ? (model.reading?.from ?? message.key) : message.key)
                        .lineLimit(1)
                        .truncationMode(.middle)
                } else {
                    Text("Open a message in the reading pane first.").foregroundStyle(.secondary)
                }
            }
            section("Own endpoint") {
                if let endpoint = model.ownAiEndpoint() {
                    Text(endpoint.baseUrl).lineLimit(1).truncationMode(.middle)
                } else {
                    Text("None is set up; the run needs one (Settings → Advanced).")
                        .foregroundStyle(.secondary)
                }
            }
            section("Models, one per line") {
                TextEditor(text: $bench.modelsText)
                    .font(.body.monospaced())
                    .frame(height: 70)
                    .border(Color.secondary.opacity(0.3))
            }
            variantsSection
            runControls
        }
        .padding(16)
    }

    private var variantsSection: some View {
        section("Instructions") {
            List(selection: $selected) {
                HStack {
                    Toggle("", isOn: $bench.includeDefault).labelsHidden()
                    Text("Default (read-only)")
                }
                .tag(TrainingVariantDraft.ID?.none)
                ForEach($bench.variants) { $variant in
                    HStack {
                        Toggle("", isOn: $variant.included).labelsHidden()
                        TextField("Name", text: $variant.name)
                    }
                    .tag(Optional(variant.id))
                }
            }
            .frame(height: 120)
            HStack {
                Button("Copy as a new variant") {
                    selected = bench.addVariant(copying: instructions(of: selected))
                }
                Button("Remove", role: .destructive) {
                    bench.variants.removeAll { $0.id == selected }
                    selected = nil
                }
                .disabled(selected == nil)
            }
            Text("Placeholders: " + (model.app?.trainingPlaceholders().map { "{\($0)}" }.joined(separator: " ") ?? ""))
                .font(.caption)
                .foregroundStyle(.secondary)
            if let index = bench.variants.firstIndex(where: { $0.id == selected }) {
                TextEditor(text: $bench.variants[index].instructions)
                    .font(.caption.monospaced())
                    .frame(minHeight: 140)
                    .border(Color.secondary.opacity(0.3))
            } else {
                ScrollView {
                    Text(defaultInstructions)
                        .font(.caption.monospaced())
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .frame(minHeight: 140)
            }
        }
    }

    private var runControls: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                if bench.running {
                    Button("Stop") { bench.stop() }
                } else {
                    Button("Run") {
                        guard let message = model.trainingMessage, let app = model.app else { return }
                        Task { await bench.run(message, app: app) }
                    }
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canRun)
                }
                Spacer()
                Button("Export…") {
                    guard let app = model.app else { return }
                    Task { await bench.export(app: app) }
                }
                .disabled(bench.running || bench.results.isEmpty)
            }
            if bench.running || bench.total > 0 {
                ProgressView(value: Double(bench.done), total: Double(max(bench.total, 1)))
                Text(bench.running ? "Drafting \(bench.done + 1) of \(bench.total): \(bench.current)"
                                   : "\(bench.done) of \(bench.total) drafted")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            if bench.exportFailed {
                Text("The export could not be written.").font(.caption).foregroundStyle(.red)
            }
        }
    }

    private var canRun: Bool {
        model.trainingMessage != nil && model.ownAiEndpoint() != nil && !bench.models.isEmpty
            && !bench.runVariants.isEmpty
    }

    private var defaultInstructions: String {
        model.app?.trainingDefaultInstructions() ?? ""
    }

    private func instructions(of variant: TrainingVariantDraft.ID?) -> String {
        bench.variants.first { $0.id == variant }?.instructions ?? defaultInstructions
    }

    private func section(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
            content()
        }
    }
}

/// Every result of the run, in the order it was drafted.
private struct TrainingResults: View {
    @Bindable var bench: TrainingBench

    var body: some View {
        if bench.results.isEmpty {
            Text(bench.running ? "Drafting…" : "Run to compare drafts here.")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 12) {
                    ForEach(bench.results.indices, id: \.self) { index in
                        TrainingResultCard(
                            result: bench.results[index],
                            feedback: $bench.ratings[index],
                            keep: { rating, _ in bench.rate(index, rating) }
                        )
                    }
                }
                .padding(16)
            }
        }
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
