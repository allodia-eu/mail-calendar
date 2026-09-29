// A drafted reply's card (docs/ai.md, "Summary and checklist" and "Where they show"): the summary,
// the checklist, what the person has done about it, and whether Send has asked. Held once per
// composer as a plain value, so its rules are tested without drawing the card.

import Foundation
import MailcalBindings

struct DraftChecklist: Equatable {
    struct Item: Equatable, Identifiable {
        let id: Int
        let kind: DraftTaskKind
        /// The placeholder exactly as the reply carries it for a fill-in item, otherwise the task.
        let text: String
        var ticked = false

        /// What the row says.
        var title: String {
            kind == .fillIn ? L10n.composer_task_fill_in(placeholder: text) : text
        }
    }

    private(set) var summary = ""
    private(set) var items: [Item] = []
    /// Which draft the card is for, so what follows the editor starts again with a later one.
    private(set) var draft = 0
    /// Whether Send has asked about open items. Once per composer, whatever the answer, so a later
    /// draft keeps it.
    private(set) var hasAsked = false
    /// The person's own collapse or expand, which a later draft keeps; `nil` until they choose.
    var collapsedChoice: Bool?
    private var attachmentsAtDraft = 0

    /// Replaces the card with a draft's, in the core's order.
    mutating func show(summary: String, tasks: [DraftTask], attachments: Int) {
        self.summary = summary
        items = tasks.enumerated().map { Item(id: $0.offset, kind: $0.element.kind, text: $0.element.text) }
        attachmentsAtDraft = attachments
        draft += 1
    }

    var isEmpty: Bool { summary.isEmpty && items.isEmpty }

    /// The placeholders the editor is asked about.
    var placeholders: [String] { items.filter { $0.kind == .fillIn }.map(\.text) }

    /// Whether a fill-in item is still open, which is what keeps the editor being asked.
    var awaitsPlaceholders: Bool { items.contains { $0.kind == .fillIn && !$0.ticked } }

    /// The draft while the editor is followed, `nil` once no fill-in item is open; a fill-in the
    /// read at Send opens again starts the following again.
    var following: Int? { awaitsPlaceholders ? draft : nil }

    /// A fill-in item is ticked exactly while its placeholder is gone from the reply, so one that
    /// comes back, by an undo, opens again.
    mutating func placeholdersLeft(_ left: [String]) {
        for index in items.indices where items[index].kind == .fillIn {
            items[index].ticked = !left.contains(items[index].text)
        }
    }

    /// The person's tick. A fill-in item follows the reply alone.
    mutating func toggle(_ id: Int) {
        guard let index = items.firstIndex(where: { $0.id == id }), items[index].kind != .fillIn else {
            return
        }
        items[index].ticked.toggle()
    }

    /// Ticks every attach item once as many files have been added since the draft as there are
    /// attach items: which file answers which item cannot be told, so fewer files tick none.
    mutating func attachmentsChanged(to count: Int) {
        let attach = items.indices.filter { items[$0].kind == .attach }
        guard !attach.isEmpty, count - attachmentsAtDraft >= attach.count else { return }
        for index in attach { items[index].ticked = true }
    }

    var openCount: Int { items.count(where: { !$0.ticked }) }

    /// Whether Send asks before it sends. It never blocks: either answer ends the asking.
    var asksBeforeSend: Bool { !hasAsked && openCount > 0 }

    mutating func sendAsked() { hasAsked = true }

    /// Collapsed when the person collapsed it, and otherwise on a phone with more than three rows.
    func isCollapsed(onPhone: Bool) -> Bool {
        collapsedChoice ?? (onPhone && items.count > 3)
    }
}
