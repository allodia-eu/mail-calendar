// What a person reads for a core error. A refused sign-in is said in their language rather than as
// the server's reply, which is a status and a body meant for a developer.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct CoreMessageTests {
    @Test func aRefusedSignInIsSaidInWords() {
        let refused = MailcalError.SignInRefused(reason: "JMAP HTTP 401: {\"status\":401}")
        #expect(coreMessage(refused) == L10n.signin_refused())
    }

    @Test func anAccountAlreadySetUpIsSaidInWords() {
        let twice = MailcalError.AlreadySetUp(accountId: "alice@example.org@imap.example.org")
        #expect(coreMessage(twice) == L10n.account_already_set_up())
    }
}
