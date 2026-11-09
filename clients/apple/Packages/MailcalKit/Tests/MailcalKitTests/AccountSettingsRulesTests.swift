// What Settings → Accounts' list says about an account, from a snapshot entry (docs/accounts.md
// rules 11 to 13), and what an edit of its servers sends back (rule 14). An account's page is the
// core's (`account_page`); `AccountPageWordsTests` covers how this client words it.

import MailcalBindings
import Testing

@testable import MailcalUI

private func linked(_ id: String) -> LinkedAccount { LinkedAccount(id: id, address: id) }

private func entry(
    _ kind: AccountKind, _ uses: [(AccountCapability, CapabilityState)]
) -> AccountEntry {
    AccountEntry(
        id: "alice@example.org@imap.example.org",
        address: "alice@example.org",
        kind: kind,
        signIn: .password,
        uses: uses.map { AccountUse(capability: $0.0, state: $0.1) },
        links: AccountLinksView(calendar: nil, contacts: nil, mail: nil),
        linkedFrom: [],
        linkCandidates: LinkCandidates(calendar: [], contacts: [], mail: [], suggested: []),
        endpoints: nil
    )
}

private func mailboxOnly() -> AccountEntry {
    entry(.imap, [(.mail, .on), (.calendar, .off), (.contacts, .off)])
}

@Suite struct AccountSettingsRulesTests {
    @Test func aUseWaitingOnPermissionIsStillOneTheAccountIsUsedFor() {
        let account = entry(
            .google,
            [(.mail, .on), (.calendar, .needsPermission), (.contacts, .off), (.colleagues, .off)])
        #expect(AccountSettingsRules.needsPermission(account))
        #expect(AccountSettingsRules.withheld(account) == [.calendar])
        // Listed as a use: it is chosen, only not granted.
        #expect(AccountSettingsRules.inUse(account) == [.mail, .calendar])
    }

    @Test func aUseTheServerDoesNotOfferIsNotOneTheAccountIsUsedFor() {
        let account = entry(.jmap, [(.mail, .on), (.calendar, .notOffered), (.contacts, .off)])
        #expect(AccountSettingsRules.inUse(account) == [.mail])
    }

    @Test func theRowNamesTheUsesInOrderAndTheLinks() {
        var mailbox = mailboxOnly()
        mailbox.links = AccountLinksView(calendar: linked("alice@cloud.example"), contacts: nil, mail: nil)
        #expect(AccountSettingsRules.inUse(mailbox) == [.mail])
        let links = AccountSettingsRules.linked(mailbox)
        #expect(links.map(\.use) == [.calendar])
        #expect(links.map(\.account.address) == ["alice@cloud.example"])
        #expect(AccountSettingsLabels.summary(mailbox).count == 2)
    }

    @Test func anAccountIsNamedByItsAddressAloneWhereNoOtherSharesIt() {
        #expect(
            AccountSettingsLabels.name(address: "alice@example.org", distinction: nil)
                == "alice@example.org")
        #expect(AccountSettingsLabels.name(linked("alice@example.org")) == "alice@example.org")
    }

    @Test func anAccountSharingItsAddressIsNamedWithItsKindOrItsServer() {
        let byKind = LinkedAccount(
            id: "alice@dav", address: "alice@example.org", distinction: .kind(kind: .dav))
        #expect(
            AccountSettingsLabels.name(byKind)
                == L10n.settings_account_distinguished(
                    address: "alice@example.org", detail: L10n.account_kind_dav()))
        let byServer = LinkedAccount(
            id: "alice@imap", address: "alice@example.org",
            distinction: .server(host: "mail.example.org"))
        #expect(
            AccountSettingsLabels.name(byServer)
                == L10n.settings_account_distinguished(
                    address: "alice@example.org", detail: "mail.example.org"))
    }

    @Test func theLinksAndTheRemovalNameAnAccountSharingItsAddress() {
        let calendar = LinkedAccount(
            id: "alice@dav", address: "alice@example.org", distinction: .kind(kind: .dav))
        let named = AccountSettingsLabels.name(calendar)
        var mailbox = mailboxOnly()
        mailbox.links = AccountLinksView(calendar: calendar, contacts: nil, mail: nil)
        #expect(
            AccountSettingsLabels.summary(mailbox)[1]
                == L10n.settings_account_linked_line(capability: L10n.nav_calendar(), address: named))

        mailbox.linkedFrom = [calendar, linked("b@example.org")]
        #expect(AccountSettingsRules.unlinked(mailbox) == "\(named), b@example.org")
    }

    @Test func theRemovalAndTheSignInNameTheAccountApartFromItsNamesake() {
        var mailbox = mailboxOnly()
        mailbox.distinction = .server(host: "mail.example.org")
        let named = L10n.settings_account_distinguished(
            address: "alice@example.org", detail: "mail.example.org")
        #expect(AccountSettingsLabels.name(mailbox) == named)
        #expect(AccountSettingsLabels.removal(mailbox) == L10n.remove_account_message(email: named))
        #expect(
            AccountSettingsLabels.signInLine(.refused, mailbox)
                == L10n.signin_expired_prompt(accounts: named))
    }

    @Test func theSignInBannerNamesAnAccountTheSnapshotHoldsWithItsDistinction() {
        var mailbox = mailboxOnly()
        mailbox.distinction = .kind(kind: .imap)
        #expect(
            AccountSettingsLabels.name(mailbox, or: "alice@example.org")
                == L10n.settings_account_distinguished(
                    address: "alice@example.org", detail: AccountSettingsLabels.kind(.imap)))
        #expect(AccountSettingsLabels.name(nil, or: "alice@example.org") == "alice@example.org")
    }

    @Test func theKindLineGainsTheHostOnlyForAServerDistinction() {
        var mailbox = mailboxOnly()
        let kind = AccountSettingsLabels.kind(.imap)
        let uses = AccountSettingsLabels.use(.mail)
        #expect(AccountSettingsLabels.kindLine(mailbox) == kind)
        #expect(AccountSettingsLabels.summary(mailbox)[0] == "\(kind) · \(uses)")

        // Its kind is already on the row, so naming it again adds nothing.
        mailbox.distinction = .kind(kind: .imap)
        #expect(AccountSettingsLabels.kindLine(mailbox) == kind)
        #expect(AccountSettingsLabels.summary(mailbox)[0] == "\(kind) · \(uses)")

        mailbox.distinction = .server(host: "mail.example.org")
        #expect(AccountSettingsLabels.kindLine(mailbox) == "\(kind) · mail.example.org")
        #expect(AccountSettingsLabels.summary(mailbox)[0] == "\(kind) · mail.example.org · \(uses)")
    }

    @Test func removingNamesTheAccountsThatLoseTheirLink() {
        var mailbox = mailboxOnly()
        #expect(AccountSettingsRules.unlinked(mailbox) == nil)
        #expect(
            AccountSettingsLabels.removal(mailbox)
                == L10n.remove_account_message(email: "alice@example.org"))

        mailbox.linkedFrom = [linked("a@example.org"), linked("b@example.org")]
        #expect(AccountSettingsRules.unlinked(mailbox) == "a@example.org, b@example.org")
        #expect(
            AccountSettingsLabels.removal(mailbox).hasSuffix(
                L10n.remove_account_unlinks(accounts: "a@example.org, b@example.org")))
    }
}

@Suite struct AccountEndpointEditTests {
    private func mailbox() -> AccountEndpoints {
        AccountEndpoints(
            imapHost: "imap.example.org", imapSecurity: .implicitTls,
            smtpHost: "smtp.example.org", smtpSecurity: .startTls,
            caldavUrl: nil, carddavUrl: nil, username: "alice", password: nil)
    }

    @Test func anUntouchedFormSendsBackWhatIsStored() {
        #expect(EndpointFields(mailbox()).edited(from: mailbox()) == mailbox())
    }

    @Test func anEmptyPasswordKeepsTheStoredOneAndATypedOneReplacesIt() {
        var fields = EndpointFields(mailbox())
        fields.password = "new secret"
        fields.imap?.field.choose(.startTls)
        let endpoints = fields.edited(from: mailbox())
        #expect(endpoints.password == "new secret")
        #expect(endpoints.imapSecurity == .startTls)
        // The port followed the security to its standard, so the host is stored bare.
        #expect(endpoints.imapHost == "imap.example.org")
    }

    @Test func aServerTheAccountDoesNotHaveIsNeverAddedByTheForm() {
        var fields = EndpointFields(mailbox())
        fields.caldav = "https://dav.example.org"
        let endpoints = fields.edited(from: mailbox())
        #expect(endpoints.caldavUrl == nil)
        #expect(endpoints.imapHost == "imap.example.org")
    }

    @Test func aStoredAddressShowsItsPortAndADefaultPortIsStoredBare() {
        let bare = EndpointServerFields(.imap, stored: "imap.example.org", security: .implicitTls)
        #expect(bare.host == "imap.example.org")
        #expect(bare.field.port == "993")
        #expect(bare.stored == "imap.example.org")

        var custom = EndpointServerFields(.imap, stored: "127.0.0.1:12993", security: .implicitTls)
        #expect(custom.host == "127.0.0.1")
        #expect(custom.field.port == "12993")
        #expect(custom.stored == "127.0.0.1:12993")
        // A port the person typed is not taken back by the picker.
        custom.field.choose(.startTls)
        #expect(custom.stored == "127.0.0.1:12993")

        // A port that is the standard one for the other security is not this one's.
        var other = EndpointServerFields(.imap, stored: "imap.example.org", security: .implicitTls)
        other.field.typePort("143")
        #expect(other.stored == "imap.example.org:143")
        other.field.choose(.startTls)
        #expect(other.stored == "imap.example.org")
    }

    @Test func aRefusedEditIsDrawnWithThePasswordItWasGiven() {
        var refused = mailbox()
        refused.password = "hunter2"
        #expect(EndpointFields(refused).password == "hunter2")
        #expect(EndpointFields(mailbox()).password.isEmpty)
    }

    @Test func aPortWrittenIntoTheHostWins() {
        // As the setup form dials it: two ports would be a contradiction, and the one in the host
        // field is the one the person can see beside the name.
        var server = EndpointServerFields(.imap, stored: "mail.example", security: .implicitTls)
        server.host = "mail.example:2993"
        server.field.typePort("1234")
        #expect(server.stored == "mail.example:2993")
    }
}
