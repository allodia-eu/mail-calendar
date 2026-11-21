// The flow host (FlowHost.swift; docs/ai.md, "The flow host"): what it pushes for a trail, that the
// system's Back reaches the record, and that it draws the record's controls where the platform
// puts them. It has no caller yet, so these are what hold it to its contract meanwhile.

import MailcalBindings
import SwiftUI
import Testing

@testable import MailcalUI

@Suite struct FlowNavigationTests {
    @Test func theFirstStageIsTheRootAndTheRestArePushed() {
        #expect(FlowNavigation.path(["account", "range", "review"]) == ["range", "review"])
        #expect(FlowNavigation.path(["range"]).isEmpty)
    }

    @Test func onlyAShorterPathIsTheSystemsBack() {
        #expect(FlowNavigation.popped(from: ["range", "review"], to: ["range"]))
        #expect(!FlowNavigation.popped(from: ["range"], to: ["range", "review"]))
        #expect(!FlowNavigation.popped(from: ["range"], to: ["review"]))
    }
}

#if os(iOS) && DEBUG
@Suite @MainActor struct FlowHostScreenTests {
    private func host(
        trail: [String], back: Bool, primary: ControlState, pressed: @escaping (String) -> Void = { _ in }
    ) -> HostedScreen {
        HostedScreen(
            FlowHost(
                trail: trail,
                title: { $0 },
                cancel: FlowControl(title: "Cancel", control: .free()) { pressed("cancel") },
                back: back ? FlowControl(title: "Back", control: .free()) { pressed("back") } : nil,
                primary: FlowControl(title: "Learn", control: primary) { pressed("learn") }
            ) { stage in
                Text("Stage \(stage)")
            })
    }

    @Test func cancelLeadsAndThePrimaryIsAtTheConfirmationPlacement() {
        let screen = host(trail: ["range"], back: false, primary: .free())
        #expect(screen.leftToRight(["Cancel", "Learn"]) == ["Cancel", "Learn"], "\(screen.dump())")
    }

    @Test func aHeldPrimaryIsDrawnHeld() {
        let screen = host(trail: ["range"], back: false, primary: .free(false))
        #expect(screen.elements("Learn").first?.enabled == false, "\(screen.dump())")
    }

    @Test func aPushedStageOffersTheSystemsBackOnlyWhenTheRecordHasOne() {
        // The system's back button: a chevron with no text, a `Button` in the tree.
        func backButtons(_ screen: HostedScreen) -> Int {
            screen.dump().split(separator: "\n").filter { $0.split(separator: " ") == ["Button", "AXButton"] }.count
        }
        let offered = host(trail: ["account", "range"], back: true, primary: .free())
        #expect(backButtons(offered) == 1, "\(offered.dump())")
        let none = host(trail: ["account", "range"], back: false, primary: .free())
        #expect(backButtons(none) == 0, "\(none.dump())")
    }
}
#endif
