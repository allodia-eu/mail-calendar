// The training window's state (docs/ai.md, "Training mode (debug builds)"): the models and the
// variants of the instructions the developer keeps, remembered in this debug build's defaults, and
// a run that drafts the pane's message with every model under every variant, one after the other.
// Compiled into a debug build for macOS only; a release build has neither this nor the core's half.

#if DEBUG && os(macOS)
import AppKit
import Foundation
import MailcalBindings
import UniformTypeIdentifiers

/// The message a run drafts replies to.
struct TrainingMessage: Equatable {
    let account: String
    let key: String
}

/// A named copy of the instructions, as the developer keeps it.
struct TrainingVariantDraft: Codable, Equatable, Identifiable {
    var id = UUID()
    var name: String
    var instructions: String
    var included = true
}

@MainActor
@Observable
final class TrainingBench {
    private static let modelsKey = "mailcal.debug.training.models"
    private static let variantsKey = "mailcal.debug.training.variants"
    private static let defaultKey = "mailcal.debug.training.includeDefault"

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

    private(set) var results: [TrainingResult] = []
    /// The rating controls of each result, beside it.
    var ratings: [DraftFeedback] = []
    private(set) var running = false
    private(set) var done = 0
    private(set) var total = 0
    /// What the run is drafting now, as "model · variant".
    private(set) var current = ""
    private var stopping = false
    /// The message the results answer, for the export.
    private(set) var answered: TrainingMessage?
    var exportFailed = false

    init() {
        let defaults = UserDefaults.standard
        modelsText = defaults.string(forKey: Self.modelsKey) ?? ""
        variants = defaults.data(forKey: Self.variantsKey)
            .flatMap { try? JSONDecoder().decode([TrainingVariantDraft].self, from: $0) } ?? []
        includeDefault = defaults.object(forKey: Self.defaultKey) as? Bool ?? true
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

    /// A copy of `instructions` to edit, under a name not yet taken.
    func addVariant(copying instructions: String) -> TrainingVariantDraft.ID {
        var index = variants.count + 1
        while variants.contains(where: { $0.name == "Variant \(index)" }) { index += 1 }
        let variant = TrainingVariantDraft(name: "Variant \(index)", instructions: instructions)
        variants.append(variant)
        return variant.id
    }

    /// Drafts `message` with every model under every variant, one after the other.
    func run(_ message: TrainingMessage, app: MailcalApp) async {
        let plan = models.flatMap { model in runVariants.map { (model, $0) } }
        guard !running, !plan.isEmpty else { return }
        running = true
        stopping = false
        answered = message
        results = []
        ratings = []
        done = 0
        total = plan.count
        let language = L10n.appLocale.identifier
        for (model, variant) in plan {
            if stopping { break }
            current = "\(model) · \(variant?.name ?? "default")"
            let result = await Task.detached(priority: .userInitiated) {
                app.trainingDraft(
                    accountId: message.account, key: message.key, model: model, variant: variant,
                    uiLanguage: language
                )
            }.value
            results.append(result)
            ratings.append(DraftFeedback())
            done += 1
        }
        current = ""
        running = false
    }

    func stop() { stopping = true }

    /// Keeps the developer's rating of result `index`.
    func rate(_ index: Int, _ rating: DraftRating) -> Bool {
        guard results.indices.contains(index) else { return false }
        results[index].rating = rating
        return true
    }

    /// Asks where to save the run, then writes it as the JSON feedback is posted in.
    func export(app: MailcalApp) async {
        guard let answered, !results.isEmpty else { return }
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.json]
        panel.nameFieldStringValue = "drafts-\(Int(Date().timeIntervalSince1970)).json"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let results = results
        let json = await Task.detached(priority: .userInitiated) {
            app.trainingExport(accountId: answered.account, key: answered.key, results: results)
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
