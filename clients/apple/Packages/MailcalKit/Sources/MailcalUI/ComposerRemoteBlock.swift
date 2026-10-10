// The content rule list behind the Apple half of the composer's "no network egress" gate
// (docs/composer-security.md): every remote (http/https) sub-resource load blocked natively, behind
// the bundle's CSP, matching Android's `shouldInterceptRequest` and Windows' `WebResourceRequested`
// 403. The composer and the signature editor both install it, through `EditorHost`.

import Foundation
import WebKit

@MainActor
enum ComposerRemoteBlock {
    /// Compiled once per process and shared. Each compile writes a new file, and the list it hands
    /// back keeps that file mapped after its web view has gone, so compiling per editor left one
    /// mapping behind for every composer opened.
    private static let compiled = SingleFlight<WKContentRuleList> { done in
        WKContentRuleListStore.default().compileContentRuleList(
            forIdentifier: "composer-block-remote",
            encodedContentRuleList: source
        ) { ruleList, _ in
            done(ruleList)
        }
    }

    private static let source = """
        [{"trigger":{"url-filter":"^https?://"},"action":{"type":"block"}}]
        """

    /// Hands back the rule list, `nil` if it could not be compiled.
    static func ruleList(_ completion: @escaping @MainActor (WKContentRuleList?) -> Void) {
        compiled.value(completion)
    }
}
