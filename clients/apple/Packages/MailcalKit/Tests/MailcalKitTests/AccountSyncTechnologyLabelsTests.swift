// What a Gmail account's page says about which way its mail syncs (docs/settings.md row 9): each
// technology its own word and explanation, and the help page it links to.

import MailcalBindings
import Testing

@testable import MailcalUI

struct AccountSyncTechnologyLabelsTests {
    private let all: [MailSyncTechnology] = [.checking, .optimised, .fallback, .unreachable]

    @Test func eachTechnologyHasAWordAndAnExplanationOfItsOwn() {
        let names = all.map(AccountSyncTechnologyLabels.name)
        let details = all.map(AccountSyncTechnologyLabels.detail)
        #expect(Set(names).count == all.count)
        #expect(Set(details).count == all.count)
        #expect(names.allSatisfy { !$0.isEmpty })
        #expect(details.allSatisfy { !$0.isEmpty })
    }

    @Test func theHelpPageHasOneAddressForEveryReader() {
        #expect(
            helpPageUrl(page: .optimiseGmailSync)
                == "https://allodia.eu/docs/mail-calendar/optimise-gmail-sync-performance")
    }
}
