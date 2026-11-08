// How this client words what the core decides on an account's page (`account_page`): what is
// wrong with a use, why its switch cannot be pressed, and what a change came to. The decisions
// themselves are the core's, tested there (docs/waiting.md, "Enforcement").

import MailcalBindings
import Testing

@testable import MailcalUI

private func toggle(
    _ capability: AccountCapability, status: AccountUseStatus? = nil,
    blocked: AccountUseBlock? = nil
) -> AccountUseSwitch {
    AccountUseSwitch(
        capability: capability, on: true, control: .free(blocked == nil), blocked: blocked,
        status: status, asksProvider: false, confirmsOff: true, reviewsServerChange: false)
}

@Suite struct AccountPageWordsTests {
    @Test func aSwitchWithNothingToSayHasNoNote() {
        #expect(AccountSettingsLabels.note(toggle(.mail)) == nil)
    }

    @Test func eachReasonASwitchCannotBePressedIsSaid() {
        let said: [(AccountUseBlock, String)] = [
            (.lastUse, L10n.settings_account_use_last()),
            (.needsContacts, L10n.settings_account_colleagues_needs_contacts()),
            (.needsMail, L10n.settings_account_shared_needs_mail()),
            (.notOffered, L10n.settings_account_use_not_offered()),
            (.needsServer, L10n.settings_account_use_needs_endpoint()),
        ]
        for (block, words) in said {
            #expect(AccountSettingsLabels.note(toggle(.calendar, blocked: block)) == words)
        }
    }

    @Test func aProblemIsSaidForTheUseItIsAbout() {
        #expect(
            AccountSettingsLabels.note(toggle(.calendar, status: .problem(problem: .serverChanged)))
                == L10n.settings_account_use_server_changed_calendar())
        #expect(
            AccountSettingsLabels.note(toggle(.contacts, status: .problem(problem: .unreachable)))
                == L10n.settings_account_use_unreachable_contacts())
        #expect(
            AccountSettingsLabels.note(toggle(.calendar, status: .withheld))
                == L10n.settings_account_use_withheld())
    }

    @Test func whatIsWrongComesBeforeWhyItCannotBePressedAndNeitherHidesTheOther() {
        let both = toggle(.calendar, status: .problem(problem: .needsOwnSignIn), blocked: .lastUse)
        #expect(
            AccountSettingsLabels.note(both)
                == L10n.settings_account_use_own_sign_in_calendar() + "\n"
                + L10n.settings_account_use_last())
    }

    @Test func aChangeThatDidNotTakeIsSaidWithTheCoresReason() {
        let use = AccountSettingsLabels.changeFailed(
            .useFailed(capability: .mail, on: false, refusal: .blocked(block: .lastUse)))
        #expect(use.title == L10n.settings_account_change_failed_title())
        #expect(use.detail == L10n.settings_account_use_last())
        let link = AccountSettingsLabels.changeFailed(
            .linkFailed(slot: .calendar, refusal: .other(detail: "gone")))
        #expect(link.detail == "gone")
        let servers = AccountSettingsLabels.changeFailed(.serversFailed(refusal: .signInRefused))
        #expect(servers.title == L10n.settings_account_servers_failed())
        #expect(servers.detail == L10n.signin_refused())
    }

    @Test func whatTheServersLeaveOutNamesTheField() {
        #expect(
            AccountSettingsLabels.endpointsGap(.smtp)
                == L10n.settings_account_servers_gap(field: L10n.setup_field_smtp()))
    }

    @Test func aHeldControlIsNotEnabledAndKeepsRunning() {
        let running = ControlState(enabled: false, running: true)
        #expect(running.held(true) == running)
        #expect(!ControlState.free().held(true).enabled)
        #expect(ControlState.free().held(false).enabled)
    }
}
