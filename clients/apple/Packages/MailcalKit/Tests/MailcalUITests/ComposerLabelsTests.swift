// The editor chrome's label payload. The cross-client key set is checked by
// `scripts/ci/check_composer_labels.py`, which reads all four clients' maps; what it cannot see is
// whether the strings actually RESOLVE. A catalog lookup that comes back empty passes every textual
// check and reaches the bundle as a key it drops, leaving that one control English in an otherwise
// translated toolbar, with nothing anywhere to say so.

import Foundation
import Testing

@testable import MailcalUI

@Suite struct ComposerLabelsTests {

    @Test func everyLabelResolvesToSomething() {
        for (key, value) in ComposerLabels.values() {
            #expect(!value.isEmpty, "editor label \(key) resolved to an empty string")
        }
    }

    @Test func theScriptIsOneWellFormedCallCarryingEveryLabel() {
        let script = ComposerLabels.script()
        #expect(script.hasPrefix("window.setComposerLabels({"))
        #expect(script.hasSuffix("})"))

        // Parse it back rather than matching text: this is the payload the bundle's `mergeLabels`
        // will see, and a translation holding a quote has to survive the round trip. If it did not,
        // the JS would be a syntax error and every label would be lost at once.
        let json = String(script.dropFirst("window.setComposerLabels(".count).dropLast())
        let decoded = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: String]
        #expect(decoded?.count == ComposerLabels.values().count)
    }

    @Test func aQuoteInATranslationIsEscapedRatherThanBreakingTheCall() {
        // Not hypothetical: the French and Italian catalog strings use typographic quotes today, and
        // a straight one is a routine thing for a translation to acquire. Interpolating unescaped
        // would end the JS string literal early.
        let payload = ["placeholder": #"a "quoted" word"#, "bold": #"back\slash"#]
        let json = try? JSONSerialization.data(withJSONObject: payload)
        let literal = String(data: json ?? Data(), encoding: .utf8) ?? ""
        #expect(literal.contains(#"\""#))
        #expect(literal.contains(#"\\"#))
    }
}
