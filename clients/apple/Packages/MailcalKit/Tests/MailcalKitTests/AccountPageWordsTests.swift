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
            (.needsMailOrCalendar, L10n.settings_account_shared_needs_mail()),
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
        let password = AccountSettingsLabels.changeFailed(.passwordFailed(refusal: .signInRefused))
        #expect(password.title == L10n.settings_account_change_failed_title())
        #expect(password.detail == L10n.signin_refused())
    }

    @Test func theSharedMailboxesSayWhatTheServerAnsweredInWords() {
        #expect(
            AccountSettingsLabels.sharedNote(.noneShared)
                == L10n.settings_account_shared_none())
        #expect(
            AccountSettingsLabels.sharedNote(.unreachable)
                == L10n.settings_account_shared_unreachable())
        let missed = AccountSettingsLabels.changeFailed(
            .sharedStoreNotFound(address: "typo@example.org"))
        #expect(missed.title == L10n.settings_account_shared_not_found())
        let gone = AccountSettingsLabels.changeFailed(.sharedStoreFailed(refusal: .noLongerShared))
        #expect(gone.detail == L10n.settings_account_shared_withdrawn())
    }

    @Test func aSharedStoreSaysItsLabelAndWhatItHolds() {
        let control = ControlState(enabled: true, running: false)
        let group = AccountSharedStore(
            handle: "s", title: "support@example.org", label: "Support Team",
            holds: .mailAndCalendar, withdrawn: false, on: false, control: control)
        #expect(
            AccountSettingsLabels.sharedStoreNote(group)
                == "Support Team · \(L10n.settings_account_shared_holds_both())")
        var dave = group
        dave.label = nil
        dave.holds = .calendar
        #expect(
            AccountSettingsLabels.sharedStoreNote(dave)
                == L10n.settings_account_shared_holds_calendar())
        dave.withdrawn = true
        #expect(
            AccountSettingsLabels.sharedStoreNote(dave)
                == L10n.settings_account_shared_withdrawn())
    }

    @Test func aBrowserSignInThatDidNotCompleteIsSaidForWhatItWasTo() {
        let again = AccountSettingsLabels.changeFailed(
            .signInFailed(switching: false, refusal: .notFinished))
        #expect(again.title == L10n.settings_account_signin_failed())
        #expect(again.detail == L10n.account_signin_timeout())
        let switching = AccountSettingsLabels.changeFailed(
            .signInFailed(switching: true, refusal: .browserNotOpened))
        #expect(switching.title == L10n.settings_account_switch_failed())
        #expect(switching.detail == L10n.account_signin_browser_failed())
        let refused = AccountSettingsLabels.changeFailed(
            .signInFailed(switching: false, refusal: .staleSignIn))
        #expect(refused.detail == L10n.signin_stale())
    }

    @Test func aSignInThatWorkedSaysOnlyWhatTheProviderStillWithholds() {
        let withheld = AccountSettingsLabels.changeFailed(
            .signedInWithheld(uses: [.calendar, .contacts]))
        #expect(withheld.title == L10n.setup_withheld_title())
        #expect(
            withheld.detail
                == L10n.setup_withheld_detail(
                    uses: AccountSettingsLabels.use(.calendar) + ", "
                        + AccountSettingsLabels.use(.contacts)))
    }

    @Test func whatTheServersLeaveOutNamesTheField() {
        #expect(
            AccountSettingsLabels.endpointsGap(.smtp)
                == L10n.settings_account_servers_gap(field: L10n.setup_field_smtp()))
    }

    @Test func aSharingChangeThatDidNotTakeIsSaidForWhy() {
        #expect(AccountSettingsLabels.sharingFailed(.unavailable) == L10n.settings_account_sync_failed())
        #expect(AccountSettingsLabels.sharingFailed(.needsReauth) == L10n.settings_allodia_reauth())
        #expect(
            AccountSettingsLabels.sharingFailed(.signedOut)
                == L10n.settings_allodia_signed_out_elsewhere())
    }

    @Test func eachSharingPositionSaysWhatItDoes() {
        #expect(AccountSettingsLabels.sharingHint(.on) == L10n.settings_account_sync_on_hint())
        #expect(AccountSettingsLabels.sharingHint(.paused) == L10n.settings_account_sync_paused_hint())
        #expect(AccountSettingsLabels.sharingHint(.off) == L10n.settings_account_sync_off_hint())
    }

    @Test func aQuestionNamesTheAccountAndWhatHappenedToIt() {
        let ask = { (elsewhere: AllodiaElsewhere) in
            AllodiaDevicesQuestion(
                accountId: "a", email: "ada@example.org", elsewhere: elsewhere, keep: .free(),
                failed: nil)
        }
        #expect(
            AccountSettingsLabels.elsewhere(ask(.changed))
                == L10n.settings_allodia_changed_elsewhere(email: "ada@example.org"))
        #expect(
            AccountSettingsLabels.elsewhere(ask(.removed))
                == L10n.settings_allodia_removed_elsewhere(email: "ada@example.org"))
    }

    @Test func aHeldControlIsNotEnabledAndKeepsRunning() {
        let running = ControlState(enabled: false, running: true)
        #expect(running.held(true) == running)
        #expect(!ControlState.free().held(true).enabled)
        #expect(ControlState.free().held(false).enabled)
    }
}
