// How the core's Sign-in section is worded, and what the "already set up" refusal keeps to update
// the account already there (docs/accounts.md rules 15 and 17).

import MailcalBindings
import Testing

@testable import MailcalUI

private func entry(_ kind: AccountKind) -> AccountEntry {
    AccountEntry(
        id: "alice@example.org@imap.example.org", address: "alice@example.org", kind: kind,
        signIn: .provider, uses: [AccountUse(capability: .mail, state: .on)],
        links: AccountLinksView(calendar: nil, contacts: nil, mail: nil), linkedFrom: [],
        linkCandidates: LinkCandidates(calendar: [], contacts: [], mail: [], suggested: []))
}

@Suite struct AccountSignInRulesTests {
    @Test func eachLineIsWorded() {
        let account = entry(.imap)
        #expect(
            AccountSettingsLabels.signInLine(.refused, account)
                == L10n.signin_expired_prompt(accounts: "alice@example.org"))
        #expect(
            AccountSettingsLabels.signInLine(.needsPermission, account)
                == L10n.settings_account_needs_permission())
        #expect(
            AccountSettingsLabels.signInLine(.atProvider, account)
                == L10n.settings_account_signin_description())
        #expect(
            AccountSettingsLabels.signInLine(.password, account)
                == L10n.settings_account_signs_in_password())
        #expect(
            AccountSettingsLabels.signInLine(.nextcloud, account)
                == L10n.settings_account_signs_in_nextcloud())
    }

    @Test func eachRouteIsWorded() {
        #expect(
            AccountSettingsLabels.signInRoute(.toProvider(label: "Fastmail"))
                == L10n.settings_account_switch_to_provider_named(provider: "Fastmail"))
        #expect(
            AccountSettingsLabels.signInRoute(.toProvider(label: nil))
                == L10n.settings_account_switch_to_provider())
        #expect(
            AccountSettingsLabels.signInRoute(.toNextcloud)
                == L10n.settings_account_switch_to_provider_named(provider: "Nextcloud"))
    }

    @Test func theRefusalOfAnAccountAlreadySetUpKeepsWhatUpdatesIt() throws {
        let refused = MailcalError.AlreadySetUp(accountId: "alice@example.org@imap.example.org")
        let offer = try #require(ExistingAccountUpdate(refusal: refused, configToml: "[imap]"))
        #expect(offer.accountId == "alice@example.org@imap.example.org")
        #expect(offer.configToml == "[imap]")

        #expect(
            ExistingAccountUpdate(
                refusal: MailcalError.SignInRefused(reason: "401"), configToml: "[imap]") == nil)
        #expect(
            ExistingAccountUpdate(refusal: JmapSignInError.couldNotStart, configToml: "[imap]") == nil)
    }
}

@Suite @MainActor struct ExistingAccountOfferTests {
    @Test func theOfferLastsOnlyAsLongAsTheAttempt() {
        let model = MailboxModel()
        model.keepExistingAccountOffer(
            MailcalError.AlreadySetUp(accountId: "alice@imap"), configToml: "[imap]")
        #expect(model.setupExistingAccount?.accountId == "alice@imap")

        model.addingAccount = true
        model.cancelSetup()
        #expect(model.setupExistingAccount == nil)

        // Another failure offers nothing.
        model.keepExistingAccountOffer(
            MailcalError.SignInRefused(reason: "401"), configToml: "[imap]")
        #expect(model.setupExistingAccount == nil)
    }
}
