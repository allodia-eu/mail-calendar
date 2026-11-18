// A SwiftUI view on a window of its own, read the way the app's test agent reads the running app
// (docs/test-agent.md): through the toolkit's accessibility objects, in this process.
//
// For a fact about one screen's layout that needs no launch, no gesture and no other process: an
// order, a fit, what is drawn at all. What a finger, a key or web content decides stays in the UI
// suite (clients/apple/UITests). This runs where MailcalKit's suite runs on iOS, which includes
// CI's (`test-ui.sh --with-kit`).
//
// ⚠️ It reads what SwiftUI draws with UIKit (a button, a field, a menu) and not a SwiftUI `Text`,
// which has an element only once the accessibility runtime is loaded into the process, as it is at
// the app's launch and not in a test host. A check that needs a text's frame stays in the UI suite.

#if os(iOS) && DEBUG
import SwiftUI
import UIKit

@testable import MailcalUI

@MainActor
final class HostedScreen {
    let window: UIWindow

    /// `view` at a phone's width, `compact` as a phone's size classes are, in `scheme`.
    init(
        _ view: some View, size: CGSize = CGSize(width: 393, height: 852),
        scheme: ColorScheme = .light, compact: Bool = true
    ) {
        let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        window = scene.map(UIWindow.init(windowScene:)) ?? UIWindow()
        window.frame = CGRect(origin: .zero, size: size)
        window.overrideUserInterfaceStyle = scheme == .dark ? .dark : .light
        let sized = view
            .environment(\.colorScheme, scheme)
            .environment(\.horizontalSizeClass, compact ? .compact : .regular)
        window.rootViewController = UIHostingController(rootView: sized)
        window.isHidden = false
        settle()
    }

    /// Lets SwiftUI lay out, measure and lay out again, as a frame or two on screen would.
    func settle() {
        for _ in 0..<10 {
            window.layoutIfNeeded()
            RunLoop.main.run(until: Date().addingTimeInterval(0.03))
        }
    }

    /// Every element on screen whose first text is `label` and whose role is `role`.
    func elements(_ label: String, role: String = "AXButton") -> [AgentNode] {
        elements(role: role) { $0 == label }
    }

    /// Every element on screen of `role` whose first text `matches`.
    func elements(role: String, matching matches: (String) -> Bool) -> [AgentNode] {
        AgentSwiftUI.switchOn(under: [window])
        var found: [AgentNode] = []
        AgentTree.walk(window) { object, _ in
            let node = AgentNode(object)
            if node.showing, node.role == role, let text = node.texts.first, matches(text) {
                found.append(node)
            }
            return true
        }
        return found
    }

    /// Every element, on screen or not, a line each as the agent's `dump` writes them: for a
    /// failure message.
    func dump() -> String {
        AgentSwiftUI.switchOn(under: [window])
        var lines: [String] = []
        AgentTree.walk(window) { object, _ in
            lines.append(AgentTree.summary(AgentNode(object), handle: false))
            return true
        }
        return lines.joined(separator: "\n")
    }

    /// The labels among `labels` that are on screen as buttons, left to right.
    func leftToRight(_ labels: [String]) -> [String] {
        labels
            .flatMap { label in elements(label).compactMap { node in node.bounds.map { (label, $0) } } }
            .sorted { $0.1.midX < $1.1.midX }
            .map(\.0)
    }
}
#endif
