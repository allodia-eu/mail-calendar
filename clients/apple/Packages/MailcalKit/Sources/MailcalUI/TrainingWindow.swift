// Develop → Compare drafts… in a macOS debug build (docs/ai.md, "Training mode (debug builds)"): the
// messages, models and variants on the left, the run's results on the right, grouped by message
// and each rated with the controls feedback uses, and the run exported as JSON. Plain English,
// because nothing here ships.

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
            ScrollView { setup }
                .frame(minWidth: 340, idealWidth: 420, maxWidth: 560)
            TrainingResults(bench: bench)
                .frame(minWidth: 420, maxWidth: .infinity, maxHeight: .infinity)
        }
        .frame(minWidth: 860, minHeight: 600)
        .task(id: lookUpKey) {
            guard bench.source == .answered, !bench.running, let app = model.app else { return }
            await bench.lookUpAnswered(app: app)
        }
    }

    private var setup: some View {
        VStack(alignment: .leading, spacing: 14) {
            messagesSection
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

    private var messagesSection: some View {
        section("Messages") {
            Picker("Messages", selection: $bench.source) {
                ForEach(TrainingSource.allCases) { Text($0.title).tag($0) }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            if bench.source == .answered {
                Stepper("The \(bench.answeredCount) most recent", value: $bench.answeredCount, in: 1...50)
            }
            let messages = runMessages
            if messages.isEmpty {
                Text(emptyMessages).foregroundStyle(.secondary)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 2) {
                        ForEach(Array(messages.enumerated()), id: \.offset) { _, message in
                            Text(label(message)).lineLimit(1).truncationMode(.tail)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .frame(maxHeight: 90)
            }
        }
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
                        guard let app = model.app else { return }
                        let messages = runMessages
                        Task { await bench.run(messages, app: app) }
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
            if bench.running || !bench.groups.isEmpty {
                let total = bench.messageCount * bench.draftsPerMessage
                ProgressView(value: Double(bench.results.count), total: Double(max(total, 1)))
                Text(progress(total: total))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            if bench.exportFailed {
                Text("The export could not be written.").font(.caption).foregroundStyle(.red)
            }
        }
    }

    /// The messages a run would answer now.
    private var runMessages: [TrainingMessage] {
        switch bench.source {
        case .selection:
            model.trainingSelection.isEmpty
                ? model.trainingMessage.map { [$0] } ?? []
                : model.trainingSelection
        case .answered:
            bench.answered
        }
    }

    private var emptyMessages: String {
        switch bench.source {
        case .selection:
            "Select messages in the list, or open one in the reading pane."
        case .answered:
            bench.lookingUp
                ? "Looking…"
                : "No answered Inbox messages found in the accounts that have a writing style."
        }
    }

    private var lookUpKey: String { "\(bench.source.rawValue)-\(bench.answeredCount)" }

    private var canRun: Bool {
        !runMessages.isEmpty && model.ownAiEndpoint() != nil && !bench.models.isEmpty
            && !bench.runVariants.isEmpty
    }

    private func progress(total: Int) -> String {
        guard bench.running else { return "\(bench.results.count) of \(total) drafted" }
        return "Message \(bench.messageNumber) of \(bench.messageCount), draft \(bench.draftNumber) "
            + "of \(bench.draftsPerMessage): \(bench.current)"
    }

    /// What the list shows of a message: its sender and subject, or what the pane shows of it
    /// when it came from the pane without them.
    private func label(_ message: TrainingMessage) -> String {
        let from = message.from.isEmpty && message.key == model.reading?.key
            ? model.reading?.from ?? "" : message.from
        let parts = [from, message.subject].filter { !$0.isEmpty }
        return parts.isEmpty ? message.key : parts.joined(separator: " · ")
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
#endif
