// How this client draws and words the links on an account's page (docs/accounts.md rule 12): a use
// a link fills, the accounts taking a use from this one, Stop linking's confirmation and what it
// sends, and the two notices about links. Which links there are is the core's, tested there.

import MailcalBindings
import Testing

@testable import MailcalUI

private let nextcloud = LinkedAccount(
    id: "cal-1", address: "alice@cloud.example", distinction: .server(host: "cloud.example"))

private func use(
    _ capability: AccountCapability, linked: LinkedAccount?, enabled: Bool = true
) -> AccountUseSwitch {
    AccountUseSwitch(
        capability: capability, on: false, control: .free(enabled), blocked: nil, status: nil,
        asksProvider: false, confirmsOff: false, reviewsServerChange: false, linked: linked)
}

private func taking(
    _ uses: [AccountCapability], leavesWithout: [AccountCapability] = []
) -> AccountLinkedFrom {
    AccountLinkedFrom(
        account: LinkedAccount(id: "mail-1", address: "alice@example.org", distinction: nil),
        uses: uses, leavesWithout: leavesWithout, stop: .free())
}

@Suite struct AccountLinkedUsesTests {
    @Test func aUseALinkFillsIsARowNamingTheAccountThatOpensItsPage() {
        let row = LinkedUseRow.of(use(.calendar, linked: nextcloud))
        #expect(
            row
                == LinkedUseRow(
                    title: L10n.nav_calendar(),
                    note: L10n.settings_account_use_linked(
                        account: L10n.settings_account_distinguished(
                            address: "alice@cloud.example", detail: "cloud.example")),
                    opens: "cal-1",
                    enabled: true))
    }

    @Test func aUseNoLinkFillsIsItsSwitch() {
        #expect(LinkedUseRow.of(use(.contacts, linked: nil)) == nil)
    }

    @Test func aLinkedUseIsHeldWhileAChangeRunsOnThePage() {
        let held = LinkedUseRow.of(use(.calendar, linked: nextcloud, enabled: false))
        #expect(held?.enabled == false)
    }

    @Test func anAccountTakingAUseSaysWhichItTakes() {
        #expect(
            AccountSettingsLabels.linkedFromLine(taking([.calendar]))
                == L10n.settings_account_linked_from_calendar())
        #expect(
            AccountSettingsLabels.linkedFromLine(taking([.contacts]))
                == L10n.settings_account_linked_from_contacts())
        #expect(
            AccountSettingsLabels.linkedFromLine(taking([.calendar, .contacts]))
                == L10n.settings_account_linked_from_both())
    }

    @Test func stoppingAsksNamingTheAccountAndSendsTheChangeForBoth() {
        let ask = StopLinkingAsk.of(taking([.calendar, .contacts]), on: "cal-1")
        #expect(ask.title == L10n.settings_account_stop_linking_title(account: "alice@example.org"))
        #expect(ask.message == L10n.settings_account_stop_linking_both())
        #expect(ask.intent == .stopLinking(account: "cal-1", from: "mail-1"))
    }

    @Test func stoppingSaysWhatTheAccountIsLeftWithoutOnlyWhenItIs() {
        #expect(
            AccountSettingsLabels.stopLinkingMessage(taking([.contacts]))
                == L10n.settings_account_stop_linking_contacts())
        #expect(
            AccountSettingsLabels.stopLinkingMessage(
                taking([.calendar], leavesWithout: [.calendar]))
                == L10n.settings_account_stop_linking_calendar() + " "
                + L10n.settings_account_stop_linking_left_without())
    }

    @Test func stopLinkingThatDidNotTakeIsSaidWithTheCoresReason() {
        let failed = AccountSettingsLabels.changeFailed(
            .stopLinkingFailed(refusal: .other(detail: "locked")))
        #expect(failed.title == L10n.settings_account_change_failed_title())
        #expect(failed.detail == "locked")
    }

    @Test func aSeparateSignInThatMovedTheUsesIsExplainedForWhatMoved() {
        let both = AccountSettingsLabels.changeFailed(
            .signedInSeparately(uses: [.calendar, .contacts]))
        #expect(both.title == L10n.settings_account_signed_in_separately_title())
        #expect(both.detail == L10n.settings_account_signed_in_separately_both())
        let contacts = AccountSettingsLabels.changeFailed(.signedInSeparately(uses: [.contacts]))
        #expect(contacts.detail == L10n.settings_account_signed_in_separately_contacts())
        let calendar = AccountSettingsLabels.changeFailed(.signedInSeparately(uses: [.calendar]))
        #expect(calendar.detail == L10n.settings_account_signed_in_separately_calendar())
    }
}
