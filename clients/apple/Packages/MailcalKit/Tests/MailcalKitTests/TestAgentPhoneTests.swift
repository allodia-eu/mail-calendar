// The iOS test agent's own rules for what VoiceOver reads (docs/test-agent.md, the macOS/iOS row),
// held where they are decided rather than only through the UI suite that leans on them.

#if os(iOS) && DEBUG
import QuartzCore
import Testing
import UIKit

@testable import MailcalUI

@Suite @MainActor struct TestAgentPhoneTests {
    /// Behind a modal view nothing is reachable; it and what is drawn over it are. A sheet's
    /// dimming view is the modal one and the sheet its next sibling, so "from the first modal one
    /// onwards", not "only the modal one".
    @Test func onlyAModalViewAndWhatIsOverItAreReached() {
        let root = UIView()
        let behind = UIView(), dimming = UIView(), sheet = UIView()
        dimming.accessibilityViewIsModal = true
        [behind, dimming, sheet].forEach(root.addSubview)

        #expect(AgentPlatform.children(of: root) == [dimming, sheet])
    }

    @Test func aFieldWithoutALabelIsNamedByItsPlaceholder() {
        let field = UITextField()
        field.placeholder = "Subject"
        #expect(AgentPlatform.texts(of: field) == ["Subject"])

        field.accessibilityLabel = "Title"
        #expect(AgentPlatform.texts(of: field) == ["Title"])
    }

    /// SwiftUI's elements answer `accessibilityIdentifier` without declaring
    /// `UIAccessibilityIdentification`, which this stands in for.
    @Test func anIdentifierIsAskedForByItsSelector() {
        final class Undeclared: NSObject {
            @objc func accessibilityIdentifier() -> String { "notice" }
        }
        #expect(AgentPlatform.identifier(of: Undeclared()) == "notice")
        #expect(AgentPlatform.identifier(of: NSObject()) == nil)
    }

    /// Sending a menu button's actions opens nothing, so the agent refuses rather than report a
    /// press that did nothing.
    @Test func aControlThatOpensAMenuIsRefused() {
        let button = UIButton(type: .system)
        button.menu = UIMenu(children: [UIAction(title: "Agenda") { _ in }])
        button.showsMenuAsPrimaryAction = true

        #expect {
            _ = try AgentPlatform.activate(button)
        } throws: { error in
            (error as? AgentFailure)?.code == "not-activatable"
        }
    }

    @Test func anAnimationThatNeverEndsIsNotMotion() {
        func animation(_ configure: (CABasicAnimation) -> Void) -> CABasicAnimation {
            let animation = CABasicAnimation(keyPath: "opacity")
            animation.duration = 0.3
            configure(animation)
            return animation
        }
        #expect(!AgentMotion.forever(animation { _ in }))
        #expect(!AgentMotion.forever(animation { $0.repeatCount = 3 }))
        #expect(AgentMotion.forever(animation { $0.repeatCount = .infinity }))
        // The caret's blink.
        #expect(AgentMotion.forever(animation { $0.repeatCount = .greatestFiniteMagnitude }))
        #expect(AgentMotion.forever(animation { $0.repeatDuration = .infinity }))
        // A system material tracking another layer.
        #expect(AgentMotion.forever(animation { $0.duration = .infinity }))
    }
}
#endif
