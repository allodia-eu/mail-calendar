// The line above the calendar week and the contacts list for a use that did not connect
// (docs/accounts.md rule 16): its three sentences, for every problem and both uses, and that it
// follows the connectivity snapshot.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct UseOutageRulesTests {
    private static let problems: [UseProblem] = [.needsOwnSignIn, .signInRefused, .unreachable]

    @Test(arguments: problems)
    func aCalendarLineIsTitleReasonAndLastSynced(_ problem: UseProblem) {
        #expect(
            UseOutageRules.text(problem, .calendar, address: "alice@example.org")
                == [
                    L10n.calendar_outage_title(address: "alice@example.org"),
                    AccountSettingsLabels.note(.problem(problem, .calendar))!,
                    L10n.use_outage_last_synced(),
                ].joined(separator: " "))
    }

    @Test(arguments: problems)
    func aContactsLineIsTitleReasonAndLastSynced(_ problem: UseProblem) {
        #expect(
            UseOutageRules.text(problem, .contacts, address: "alice@example.org")
                == [
                    L10n.contacts_outage_title(address: "alice@example.org"),
                    AccountSettingsLabels.note(.problem(problem, .contacts))!,
                    L10n.use_outage_last_synced(),
                ].joined(separator: " "))
    }

    @Test func theReasonIsTheOneSettingsGivesForThatUse() {
        let reasons: [(UseProblem, OutageUse, String)] = [
            (.needsOwnSignIn, .calendar, L10n.settings_account_use_own_sign_in_calendar()),
            (.signInRefused, .calendar, L10n.settings_account_use_refused_calendar()),
            (.unreachable, .calendar, L10n.settings_account_use_unreachable_calendar()),
            (.needsOwnSignIn, .contacts, L10n.settings_account_use_own_sign_in_contacts()),
            (.signInRefused, .contacts, L10n.settings_account_use_refused_contacts()),
            (.unreachable, .contacts, L10n.settings_account_use_unreachable_contacts()),
        ]
        for (problem, use, reason) in reasons {
            #expect(UseOutageRules.text(problem, use, address: "a@b.test").contains(reason))
        }
    }

    @Test func oneLinePerAccountInTheSnapshotsOrderNamedByAddress() {
        let outages = [
            UseOutage(accountId: "b-id", problem: .unreachable),
            UseOutage(accountId: "a-id", problem: .needsOwnSignIn),
        ]
        let addresses = ["a-id": "a@example.org", "b-id": "b@example.org"]
        let lines = UseOutageRules.lines(outages, .calendar) { addresses[$0] ?? $0 }
        #expect(lines.map(\.account) == ["b-id", "a-id"])
        #expect(lines[0].text == UseOutageRules.text(.unreachable, .calendar, address: "b@example.org"))
        #expect(lines[1].text == UseOutageRules.text(.needsOwnSignIn, .calendar, address: "a@example.org"))
    }
}

@Suite @MainActor struct UseOutageSnapshotTests {
    private func snapshot(calendar: [UseOutage] = [], contacts: [UseOutage] = []) -> ConnectivitySnapshot {
        ConnectivitySnapshot(
            offline: false,
            unreachableAccounts: [],
            calendarReauthAccounts: [],
            mailReauthAccounts: [],
            signinExpiredAccounts: [],
            calendarOutages: calendar,
            contactsOutages: contacts
        )
    }

    @Test func aSnapshotWithOutagesDrawsALineForEachUse() {
        let model = MailboxModel()
        model.accounts = [AccountRow(id: "alice-id", email: "alice@example.org", name: "", expanded: true)]
        model.connectivity = snapshot(
            calendar: [UseOutage(accountId: "alice-id", problem: .needsOwnSignIn)],
            contacts: [UseOutage(accountId: "alice-id", problem: .signInRefused)]
        )
        #expect(
            model.calendarOutageLines
                == [UseOutageLine(
                    account: "alice-id",
                    text: UseOutageRules.text(.needsOwnSignIn, .calendar, address: "alice@example.org"))])
        #expect(
            model.contactsOutageLines
                == [UseOutageLine(
                    account: "alice-id",
                    text: UseOutageRules.text(.signInRefused, .contacts, address: "alice@example.org"))])
    }

    @Test func aSnapshotWithoutOutagesDrawsNone() {
        let model = MailboxModel()
        #expect(model.calendarOutageLines.isEmpty)
        model.connectivity = snapshot()
        #expect(model.calendarOutageLines.isEmpty)
        #expect(model.contactsOutageLines.isEmpty)
    }

    @Test func aLineNeverShowsTheIdOfAnAccountItCanName() {
        let model = MailboxModel()
        model.accounts = [
            AccountRow(id: "alice@example.org@jmap:host", email: "alice@example.org", name: "", expanded: true),
        ]
        model.connectivity = snapshot(
            calendar: [UseOutage(accountId: "alice@example.org@jmap:host", problem: .unreachable)])
        #expect(!model.calendarOutageLines[0].text.contains("jmap:host"))
    }
}
