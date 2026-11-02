// What a Gmail account's page says about which way its mail syncs (docs/settings.md row 9): each
// technology its own word and explanation, and the help page in the reader's language where the
// help site has one.

import MailcalBindings
import Testing

@testable import MailcalUI

struct AccountSyncTechnologyLabelsTests {
    private let all: [MailSyncTechnology] = [.checking, .optimised, .fallback]

    @Test func eachTechnologyHasAWordAndAnExplanationOfItsOwn() {
        let names = all.map(AccountSyncTechnologyLabels.name)
        let details = all.map(AccountSyncTechnologyLabels.detail)
        #expect(Set(names).count == all.count)
        #expect(Set(details).count == all.count)
        #expect(names.allSatisfy { !$0.isEmpty })
        #expect(details.allSatisfy { !$0.isEmpty })
    }

    @Test func theHelpPageFollowsTheReaderWhereItIsWritten() {
        let english = helpPageUrl(page: .optimiseGmailSync, locale: "en")
        #expect(english == "https://allodia.eu/docs/mail-calendar/optimise-gmail-sync-performance")
        #expect(helpPageUrl(page: .optimiseGmailSync, locale: "nl").contains("/nl/docs/"))
        #expect(helpPageUrl(page: .optimiseGmailSync, locale: "de") == english)
    }
}
