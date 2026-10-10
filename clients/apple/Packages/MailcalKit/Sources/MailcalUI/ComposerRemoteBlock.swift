// The content rule list behind the Apple half of the composer's "no network egress" gate
// (docs/composer-security.md): every remote (http/https) sub-resource load blocked natively, behind
// the bundle's CSP, matching Android's `shouldInterceptRequest` and Windows' `WebResourceRequested`
// 403. The composer and the signature editor both install it.

import Foundation
import WebKit

@MainActor
enum ComposerRemoteBlock {
    /// Compiled once per process and shared. Each compile writes a new file, and the list it hands
    /// back keeps that file mapped after its web view has gone, so compiling per editor left one
    /// mapping behind for every composer opened.
    private static var compiled: WKContentRuleList?
    /// The callers waiting on a compile already under way, so two editors opened together share one
    /// compile rather than each writing a file and keeping their own.
    private static var waiting: [@MainActor (WKContentRuleList?) -> Void]?

    private static let source = """
        [{"trigger":{"url-filter":"^https?://"},"action":{"type":"block"}}]
        """

    /// Hands back the rule list, `nil` if it could not be compiled. Always asynchronous, so a
    /// caller sees the same order whether or not it was compiled already.
    static func ruleList(_ completion: @escaping @MainActor (WKContentRuleList?) -> Void) {
        if let compiled {
            Task { @MainActor in completion(compiled) }
            return
        }
        if waiting != nil {
            waiting?.append(completion)
            return
        }
        waiting = [completion]
        WKContentRuleListStore.default().compileContentRuleList(
            forIdentifier: "composer-block-remote",
            encodedContentRuleList: source
        ) { ruleList, _ in
            compiled = ruleList
            let callers = waiting ?? []
            waiting = nil
            for caller in callers {
                caller(ruleList)
            }
        }
    }
}
