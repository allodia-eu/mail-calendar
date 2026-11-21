// The words Settings → Advanced puts to the core's record: what the assistant server is doing, and
// what a reset came to. Which status applies, and when, is the core's, tested there.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct AdvancedWordsTests {
    @Test func eachStatusHasItsOwnWords() {
        #expect(McpStatusWords.text(.off) == L10n.settings_mcp_status_off())
        #expect(McpStatusWords.text(.starting) == L10n.settings_mcp_status_starting())
        #expect(McpStatusWords.text(.listening) == L10n.settings_mcp_status_running())
        #expect(
            McpStatusWords.text(.anotherInstance) == L10n.settings_mcp_status_another_instance())
        #expect(McpStatusWords.text(.unavailable) == L10n.settings_mcp_status_unavailable())
    }

    /// Only a server that was switched on and cannot be reached is drawn as a warning; one still
    /// binding its endpoint is not.
    @Test func onlyAServerNobodyCanReachWarns() {
        #expect(McpStatusWords.warns(.anotherInstance))
        #expect(McpStatusWords.warns(.unavailable))
        #expect(!McpStatusWords.warns(.off))
        #expect(!McpStatusWords.warns(.starting))
        #expect(!McpStatusWords.warns(.listening))
    }

    @Test func aResetSaysWhatItCameTo() {
        #expect(ResetOutcomeWords.text(.done) == L10n.settings_advanced_reset_done())
        #expect(ResetOutcomeWords.text(.failed) == L10n.settings_advanced_reset_failed())
    }

    @Test func beforeTheCoreOpensThereIsNoPanelAndNothingToPress() {
        let page = AdvancedPage.empty
        #expect(page.assistants == nil)
        #expect(!page.reset.enabled && !page.reset.running)
        #expect(page.resetOutcome == nil)
    }
}
