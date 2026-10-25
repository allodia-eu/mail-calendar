// What Settings → Accounts says about an account, from a snapshot entry (docs/accounts.md rules
// 11 to 13), and what an edit of its servers sends back (rule 14). The Apple client's copy of the
// suite Linux (`account_rows_tests`, `account_servers`) and Windows (`AccountSettingsRulesTests`,
// `AccountEndpointEditTests`) carry.

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
    @Test func aUseThatDidNotConnectSaysWhyBeneathItsSwitch() {
        var account = entry(.imap, [(.mail, .on), (.calendar, .on), (.contacts, .on)])
        account.uses[1] = AccountUse(capability: .calendar, state: .on, problem: .needsOwnSignIn)
        account.uses[2] = AccountUse(capability: .contacts, state: .on, problem: .unreachable)
        let calendar = AccountSettingsRules.useSwitch(account, .calendar)
        #expect(calendar.note == .problem(.needsOwnSignIn, .calendar))
        #expect(calendar.active)
        #expect(
            AccountSettingsLabels.note(calendar.note)
                == L10n.settings_account_use_own_sign_in_calendar())
        #expect(
            AccountSettingsRules.useSwitch(account, .contacts).note == .problem(.unreachable, .contacts))
        #expect(AccountSettingsRules.useSwitch(account, .mail).note == UseNote.none)
    }

    @Test func aUsePausedForANewServerSaysSoAndOffersReview() {
        var account = entry(.dav, [(.calendar, .on), (.contacts, .on)])
        account.uses[0] = AccountUse(capability: .calendar, state: .on, problem: .serverChanged)
        account.uses[1] = AccountUse(capability: .contacts, state: .on, problem: .serverChanged)
        let calendar = AccountSettingsRules.useSwitch(account, .calendar).note
        let contacts = AccountSettingsRules.useSwitch(account, .contacts).note
        #expect(calendar == .problem(.serverChanged, .calendar))
        #expect(
            AccountSettingsLabels.note(calendar)
                == L10n.settings_account_use_server_changed_calendar())
        #expect(
            AccountSettingsLabels.note(contacts)
                == L10n.settings_account_use_server_changed_contacts())
        #expect(AccountSettingsRules.reviewsServerChange(calendar))
        #expect(AccountSettingsRules.reviewsServerChange(contacts))
    }

    @Test func onlyAServerChangeOffersReview() {
        let notes: [UseNote] = [
            .none, .last, .needsContacts, .withheld, .notOffered,
            .problem(.needsOwnSignIn, .calendar), .problem(.signInRefused, .contacts),
            .problem(.unreachable, .calendar),
        ]
        for note in notes {
            #expect(!AccountSettingsRules.reviewsServerChange(note))
        }
    }

    @Test func theLastUseCannotBeSwitchedOff() {
        let mailbox = mailboxOnly()
        #expect(
            AccountSettingsRules.useSwitch(mailbox, .mail)
                == UseSwitch(active: true, asks: false, enabled: false, note: .last))
        #expect(
            AccountSettingsRules.useSwitch(mailbox, .calendar)
                == UseSwitch(active: false, asks: false, enabled: true, note: .none))
    }

    @Test func colleaguesWaitForContacts() {
        var account = entry(
            .microsoft, [(.mail, .on), (.calendar, .on), (.contacts, .off), (.colleagues, .off)])
        let colleagues = AccountSettingsRules.useSwitch(account, .colleagues)
        #expect(!colleagues.enabled)
        #expect(colleagues.note == .needsContacts)

        account.uses[2] = AccountUse(capability: .contacts, state: .on)
        #expect(AccountSettingsRules.useSwitch(account, .colleagues).enabled)
    }

    @Test func aUseWaitingOnPermissionIsDrawnOffAndSwitchingItOnAsks() {
        let account = entry(
            .google,
            [(.mail, .on), (.calendar, .needsPermission), (.contacts, .off), (.colleagues, .off)])
        let calendar = AccountSettingsRules.useSwitch(account, .calendar)
        #expect(!calendar.active)
        #expect(calendar.asks && calendar.enabled)
        #expect(calendar.note == .withheld)
        #expect(AccountSettingsRules.needsPermission(account))
        #expect(AccountSettingsRules.withheld(account) == [.calendar])
        // Listed as a use: it is chosen, only not granted.
        #expect(AccountSettingsRules.inUse(account).contains(.calendar))
    }

    @Test func anOnlyUseWaitingOnPermissionCanStillBeAskedFor() {
        let account = entry(.google, [(.mail, .off), (.calendar, .needsPermission), (.contacts, .off)])
        #expect(
            AccountSettingsRules.useSwitch(account, .calendar)
                == UseSwitch(active: false, asks: true, enabled: true, note: .withheld))
    }

    @Test func aUseTheServerDoesNotOfferCannotBeSwitchedAndHoldsNothingOn() {
        let account = entry(.jmap, [(.mail, .on), (.calendar, .notOffered), (.contacts, .off)])
        #expect(
            AccountSettingsRules.useSwitch(account, .calendar)
                == UseSwitch(active: false, asks: false, enabled: false, note: .notOffered))
        // Mail is the one use that works, so it is the last.
        #expect(AccountSettingsRules.useSwitch(account, .mail).note == .last)
        #expect(AccountSettingsRules.inUse(account) == [.mail])
    }

    @Test func signingInAgainIsOfferedWhereTheProviderSignsIn() {
        #expect(AccountSettingsRules.signsInAtProvider(.microsoft))
        #expect(AccountSettingsRules.signsInAtProvider(.google))
        for kind in [AccountKind.imap, .dav, .jmap] {
            #expect(!AccountSettingsRules.signsInAtProvider(kind))
        }
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
            AccountSettingsLabels.signIn(mailbox, expired: true)
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

    @Test func aPickerIsOfferedOnlyForASlotTheAccountCanHold() {
        var mailbox = mailboxOnly()
        #expect(AccountSettingsRules.linkPickers(mailbox).isEmpty)

        mailbox.linkCandidates = LinkCandidates(
            calendar: [linked("a@cloud.example"), linked("b@cloud.example")], contacts: [], mail: [],
            suggested: [])
        mailbox.links = AccountLinksView(calendar: linked("b@cloud.example"), contacts: nil, mail: nil)
        let pickers = AccountSettingsRules.linkPickers(mailbox)
        #expect(pickers.count == 1)
        #expect(pickers.first?.slot == .calendar)
        #expect(pickers.first?.options.count == 2)
        #expect(pickers.first?.selected == 1)
    }

    @Test func aLinkedAccountMissingFromTheCandidatesIsStillShown() {
        var mailbox = mailboxOnly()
        mailbox.links = AccountLinksView(calendar: nil, contacts: linked("book@cloud.example"), mail: nil)
        let pickers = AccountSettingsRules.linkPickers(mailbox)
        #expect(pickers.count == 1)
        #expect(pickers.first?.slot == .contacts)
        #expect(pickers.first?.options == [linked("book@cloud.example")])
        #expect(pickers.first?.selected == 0)
    }

    @Test func aLinkIsMatchedToItsCandidateByIdAlone() {
        var mailbox = mailboxOnly()
        mailbox.linkCandidates = LinkCandidates(
            calendar: [LinkedAccount(id: "cal", address: "new@cloud.example")], contacts: [],
            mail: [], suggested: [])
        mailbox.links = AccountLinksView(
            calendar: LinkedAccount(id: "cal", address: "old@cloud.example"), contacts: nil, mail: nil)
        let picker = AccountSettingsRules.linkPickers(mailbox).first
        #expect(picker?.options.map(\.id) == ["cal"])
        #expect(picker?.selected == 0)
    }

    @Test func aSuggestionIsOfferedOnlyWhereNothingIsLinked() {
        var mailbox = mailboxOnly()
        mailbox.linkCandidates = LinkCandidates(
            calendar: [linked("a@cloud.example"), linked("b@cloud.example")], contacts: [], mail: [],
            suggested: ["b@cloud.example"])
        #expect(AccountSettingsRules.linkPickers(mailbox).first?.suggested == 1)

        mailbox.links = AccountLinksView(calendar: linked("a@cloud.example"), contacts: nil, mail: nil)
        // A link already made is not second-guessed.
        #expect(AccountSettingsRules.linkPickers(mailbox).first?.suggested == nil)
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

    @Test func aNewSecretIsOfferedOnlyForAnExpiredPastedOne() {
        let jmap = entry(.jmap, [(.mail, .on)])
        #expect(AccountSettingsRules.replacesSecret(jmap, expired: true, provider: .jmap))
        #expect(!AccountSettingsRules.replacesSecret(jmap, expired: false, provider: .jmap))
        // Signed in at a provider: signing in again is the remedy, not a pasted secret.
        #expect(!AccountSettingsRules.replacesSecret(jmap, expired: true, provider: .jmapOauth))

        // Editable servers take the new password in their own section.
        var imap = entry(.imap, [(.mail, .on)])
        imap.endpoints = AccountEndpoints(
            imapHost: "imap.example.org", imapSecurity: .implicitTls, smtpHost: nil,
            smtpSecurity: .implicitTls, caldavUrl: nil, carddavUrl: nil, username: "alice",
            password: nil)
        #expect(!AccountSettingsRules.replacesSecret(imap, expired: true, provider: .password))

        // An IMAP account that signed in at its provider has no editable servers, and no secret a
        // paste could replace.
        let imapSignedIn = entry(.imap, [(.mail, .on)])
        #expect(!AccountSettingsRules.replacesSecret(imapSignedIn, expired: true, provider: .imapOauth))
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

    @Test func savingWaitsWhileAServerTheAccountHasIsEmpty() {
        var fields = EndpointFields(mailbox())
        #expect(!fields.incomplete)
        fields.smtp?.host = "  "
        #expect(fields.incomplete)
        fields.smtp?.host = "smtp.example.org"
        fields.username = ""
        #expect(fields.incomplete)
        // A server the account does not have has no field, and is never what holds Save.
        fields.username = "alice"
        #expect(fields.caldav == nil && !fields.incomplete)
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
