// A drafted reply's card (docs/ai.md, "Summary and checklist" and "Where they show"): the items in
// the core's order, a fill-in item that follows the reply and nothing else, the others ticked by the
// person, and Send asking once per composer while anything is open.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct DraftChecklistTests {
    private static let tasks = [
        DraftTask(kind: .fillIn, text: "[date]"),
        DraftTask(kind: .fillIn, text: "[time]"),
        DraftTask(kind: .attach, text: "Attach the agenda"),
        DraftTask(kind: .do, text: "Book the room"),
    ]

    private func drafted(_ tasks: [DraftTask] = tasks, attachments: Int = 0) -> DraftChecklist {
        var checklist = DraftChecklist()
        checklist.show(summary: "Bob asks when you can meet.", tasks: tasks, attachments: attachments)
        return checklist
    }

    @Test func itemsKeepTheCoresOrderAndALaterDraftReplacesThem() {
        var checklist = drafted()
        #expect(checklist.items.map(\.text) == ["[date]", "[time]", "Attach the agenda", "Book the room"])
        #expect(checklist.items.map(\.title).first == L10n.composer_task_fill_in(placeholder: "[date]"))
        #expect(checklist.items.last?.title == "Book the room")
        checklist.toggle(3)
        let first = checklist.draft
        checklist.show(summary: "", tasks: [DraftTask(kind: .do, text: "Call Bob")], attachments: 0)
        #expect(checklist.items.map(\.text) == ["Call Bob"])
        #expect(checklist.openCount == 1)
        #expect(checklist.summary.isEmpty)
        #expect(checklist.draft != first)
    }

    @Test func aDraftWithNothingToSayShowsNoCard() {
        #expect(DraftChecklist().isEmpty)
        var checklist = DraftChecklist()
        checklist.show(summary: "", tasks: [], attachments: 0)
        #expect(checklist.isEmpty)
        checklist.show(summary: "Bob asks for the slides.", tasks: [], attachments: 0)
        #expect(!checklist.isEmpty)
    }

    /// Ticked exactly while the placeholder is gone, so an undo opens the item again.
    @Test func aFillInItemFollowsWhatIsLeftInTheReply() {
        var checklist = drafted()
        #expect(checklist.placeholders == ["[date]", "[time]"])
        #expect(checklist.awaitsPlaceholders)
        checklist.placeholdersLeft(["[time]"])
        #expect(checklist.items.map(\.ticked) == [true, false, false, false])
        #expect(checklist.following == checklist.draft)
        checklist.placeholdersLeft([])
        #expect(!checklist.awaitsPlaceholders)
        #expect(checklist.following == nil)
        checklist.placeholdersLeft(["[date]"])
        #expect(checklist.items.map(\.ticked) == [false, true, false, false])
        #expect(checklist.following == checklist.draft, "a reopened item is followed again")
    }

    @Test func onlyTheItemsTheEditorCannotSeeAreTickedByHand() {
        var checklist = drafted()
        checklist.toggle(0)
        #expect(!checklist.items[0].ticked)
        checklist.toggle(2)
        checklist.toggle(3)
        #expect(checklist.items[2].ticked && checklist.items[3].ticked)
        checklist.toggle(3)
        #expect(!checklist.items[3].ticked)
        checklist.placeholdersLeft([])
        #expect(checklist.items[2].ticked, "the reply leaves an attach item alone")
    }

    /// Which file answers which item cannot be told, so an attach item ticks only once there is a
    /// new file for every one of them.
    @Test func attachItemsTickOnceEachHasANewFile() {
        let tasks = [
            DraftTask(kind: .attach, text: "Attach the agenda"),
            DraftTask(kind: .attach, text: "Attach the minutes"),
            DraftTask(kind: .do, text: "Book the room"),
        ]
        var checklist = drafted(tasks, attachments: 1)
        checklist.attachmentsChanged(to: 2)
        #expect(checklist.items.map(\.ticked) == [false, false, false])
        checklist.attachmentsChanged(to: 3)
        #expect(checklist.items.map(\.ticked) == [true, true, false])

        var none = drafted([DraftTask(kind: .do, text: "Book the room")])
        none.attachmentsChanged(to: 4)
        #expect(!none.items[0].ticked)
    }

    @Test func theOpenCountIsEveryUntickedItem() {
        var checklist = drafted()
        #expect(checklist.openCount == 4)
        checklist.placeholdersLeft(["[time]"])
        checklist.toggle(2)
        #expect(checklist.openCount == 2)
    }

    /// Either answer ends the asking for the composer, a later draft included; and with nothing
    /// open it never asks at all.
    @Test func sendAsksOnceAndOnlyWhileSomethingIsOpen() {
        var checklist = drafted()
        #expect(checklist.asksBeforeSend)
        checklist.sendAsked()
        #expect(!checklist.asksBeforeSend)
        checklist.show(summary: "", tasks: Self.tasks, attachments: 0)
        #expect(checklist.hasAsked && !checklist.asksBeforeSend)

        var done = drafted([DraftTask(kind: .do, text: "Book the room")])
        done.toggle(0)
        #expect(!done.asksBeforeSend)
        #expect(!DraftChecklist().asksBeforeSend)
    }

    @Test func aPhoneCollapsesALongListUntilThePersonChooses() {
        var checklist = drafted()
        #expect(checklist.isCollapsed(onPhone: true))
        #expect(!checklist.isCollapsed(onPhone: false))
        #expect(!drafted(Array(Self.tasks.prefix(3))).isCollapsed(onPhone: true))
        checklist.collapsedChoice = false
        checklist.show(summary: "", tasks: Self.tasks, attachments: 0)
        #expect(!checklist.isCollapsed(onPhone: true), "the choice outlives a later draft")
    }
}
