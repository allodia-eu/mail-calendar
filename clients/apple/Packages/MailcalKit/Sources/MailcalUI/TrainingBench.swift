// The training window's state (docs/ai.md, "Training mode (debug builds)"): the models and the
// variants of the instructions the developer keeps, remembered in this debug build's defaults, and
// a run that drafts every chosen message with every model under every variant, one after the
// other. Compiled into a debug build for macOS only; a release build has neither this nor the
// core's half.

#if DEBUG && os(macOS)
import AppKit
import Foundation
import MailcalBindings
import UniformTypeIdentifiers

extension TrainingMessage {
    /// A list row as a run answers it: a conversation by its latest message.
    init(_ row: SnapshotRow) {
        switch row {
        case .flat(let message):
            self.init(accountId: message.account, key: message.key, from: message.from,
                      subject: message.subject)
        case .thread(let thread):
            self.init(accountId: thread.account, key: thread.latestKey, from: thread.latestFrom,
                      subject: thread.subject)
        }
    }
}

/// A named copy of the instructions, as the developer keeps it.
struct TrainingVariantDraft: Codable, Equatable, Identifiable {
    var id = UUID()
    var name: String
    var instructions: String
    var included = true
}

/// Where a run's messages come from.
enum TrainingSource: String, CaseIterable, Identifiable {
    /// The rows selected in the list, or the message open in the pane when none is.
    case selection
    /// The newest Inbox messages the person answered, in the accounts that have a writing style.
    case answered

    var id: Self { self }

    var title: String {
        switch self {
        case .selection: "Selected in the list"
        case .answered: "Inbox messages I answered"
        }
    }
}

/// One message of a run and what was drafted for it, in order.
struct TrainingGroup: Identifiable {
    /// Its place in the run.
    let id: Int
    let message: TrainingMessage
    var results: [TrainingResult] = []
    /// The rating controls of each result, beside it.
    var ratings: [DraftFeedback] = []
}

@MainActor
@Observable
final class TrainingBench {
    private static let modelsKey = "mailcal.debug.training.models"
    private static let variantsKey = "mailcal.debug.training.variants"
    private static let defaultKey = "mailcal.debug.training.includeDefault"
    private static let sourceKey = "mailcal.debug.training.source"
    private static let answeredCountKey = "mailcal.debug.training.answeredCount"

    /// One model name per line, as typed.
    var modelsText: String {
        didSet { UserDefaults.standard.set(modelsText, forKey: Self.modelsKey) }
    }
    var variants: [TrainingVariantDraft] {
        didSet {
            if let data = try? JSONEncoder().encode(variants) {
                UserDefaults.standard.set(data, forKey: Self.variantsKey)
            }
        }
    }
    /// Whether a run drafts under the default instructions too.
    var includeDefault: Bool {
        didSet { UserDefaults.standard.set(includeDefault, forKey: Self.defaultKey) }
    }
    var source: TrainingSource {
        didSet { UserDefaults.standard.set(source.rawValue, forKey: Self.sourceKey) }
    }
    /// How many answered Inbox messages a run takes.
    var answeredCount: Int {
        didSet { UserDefaults.standard.set(answeredCount, forKey: Self.answeredCountKey) }
    }

    /// The answered Inbox messages, as last looked up.
    private(set) var answered: [TrainingMessage] = []
    private(set) var lookingUp = false

    var groups: [TrainingGroup] = []
    /// How each model did under each variant so far, as the core counts it.
    private(set) var summary: [TrainingSummary] = []
    private(set) var running = false
    /// Which message the run is on, and of how many.
    private(set) var messageNumber = 0
    private(set) var messageCount = 0
    /// Which draft of the message the run is on, and of how many.
    private(set) var draftNumber = 0
    private(set) var draftsPerMessage = 0
    /// What the run is drafting now, as "model · variant".
    private(set) var current = ""
    private var stopping = false
    var exportFailed = false

    init() {
        let defaults = UserDefaults.standard
        modelsText = defaults.string(forKey: Self.modelsKey) ?? ""
        variants = defaults.data(forKey: Self.variantsKey)
            .flatMap { try? JSONDecoder().decode([TrainingVariantDraft].self, from: $0) } ?? []
        includeDefault = defaults.object(forKey: Self.defaultKey) as? Bool ?? true
        source = defaults.string(forKey: Self.sourceKey).flatMap(TrainingSource.init) ?? .selection
        answeredCount = defaults.object(forKey: Self.answeredCountKey) as? Int ?? 5
    }

    var models: [String] {
        modelsText.split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
    }

    /// Each variant a run drafts under: `nil` for the default instructions.
    var runVariants: [TrainingVariant?] {
        (includeDefault ? [nil] : [])
            + variants.filter(\.included).map { TrainingVariant(name: $0.name, instructions: $0.instructions) }
    }

    /// Every result of the run, in the order drafted.
    var results: [TrainingResult] { groups.flatMap(\.results) }

    /// A copy of `instructions` to edit, under a name not yet taken.
    func addVariant(copying instructions: String) -> TrainingVariantDraft.ID {
        var index = variants.count + 1
        while variants.contains(where: { $0.name == "Variant \(index)" }) { index += 1 }
        let variant = TrainingVariantDraft(name: "Variant \(index)", instructions: instructions)
        variants.append(variant)
        return variant.id
    }

    /// Looks up the newest answered Inbox messages, as many as `answeredCount`. A look-up the
    /// window has since replaced with another keeps nothing.
    func lookUpAnswered(app: MailcalApp) async {
        lookingUp = true
        let count = UInt32(max(answeredCount, 1))
        let found = await Task.detached(priority: .userInitiated) {
            app.trainingAnsweredMessages(count: count)
        }.value
        guard !Task.isCancelled else { return }
        answered = found
        lookingUp = false
    }

    /// Drafts every message with every model under every variant, one after the other.
    func run(_ messages: [TrainingMessage], app: MailcalApp) async {
        let plan = models.flatMap { model in runVariants.map { (model, $0) } }
        guard !running, !plan.isEmpty, !messages.isEmpty else { return }
        running = true
        stopping = false
        groups = messages.enumerated().map { TrainingGroup(id: $0.offset, message: $0.element) }
        summary = []
        messageCount = messages.count
        draftsPerMessage = plan.count
        let language = L10n.appLocale.identifier
        drafting: for (index, message) in messages.enumerated() {
            messageNumber = index + 1
            for (step, (model, variant)) in plan.enumerated() {
                if stopping { break drafting }
                draftNumber = step + 1
                current = "\(model) · \(variant?.name ?? "default")"
                let result = await Task.detached(priority: .userInitiated) {
                    app.trainingDraft(
                        accountId: message.accountId, key: message.key, model: model,
                        variant: variant, uiLanguage: language
                    )
                }.value
                groups[index].results.append(result)
                groups[index].ratings.append(DraftFeedback())
                summary = app.trainingSummary(results: results)
            }
        }
        current = ""
        running = false
    }

    func stop() { stopping = true }

    /// Keeps the developer's rating of result `index` of message `group`.
    func rate(_ group: Int, _ index: Int, _ rating: DraftRating) -> Bool {
        guard groups.indices.contains(group), groups[group].results.indices.contains(index) else {
            return false
        }
        groups[group].results[index].rating = rating
        return true
    }

    /// Asks where to save the run, then writes it as one JSON document: every message with its
    /// results and ratings, and the summary.
    func export(app: MailcalApp) async {
        let answered = groups.filter { !$0.results.isEmpty }
            .map { TrainingMessageResults(message: $0.message, results: $0.results) }
        guard !answered.isEmpty else { return }
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.json]
        panel.nameFieldStringValue = "drafts-\(Int(Date().timeIntervalSince1970)).json"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let json = await Task.detached(priority: .userInitiated) {
            app.trainingExport(messages: answered)
        }.value
        do {
            try json.write(to: url, atomically: true, encoding: .utf8)
            exportFailed = false
        } catch {
            exportFailed = true
        }
    }
}
#endif
