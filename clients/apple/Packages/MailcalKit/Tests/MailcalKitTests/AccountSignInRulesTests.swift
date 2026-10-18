// How a standards or JMAP account's page offers to switch its sign-in, which remedy the banner
// takes for each family, and what the "already set up" refusal keeps to update the account
// already there (docs/accounts.md rules 15 and 17).

import MailcalBindings
import Testing

@testable import MailcalUI

private func options(signIn: Bool, label: String? = nil, password: Bool) -> AccountSignInOptions {
    AccountSignInOptions(signIn: signIn, providerLabel: label, password: password)
}

@Suite struct AccountSignInRulesTests {
    @Test func nothingIsOfferedBeforeTheServerAnswers() {
        #expect(AccountSignInRules.actions(.provider, nil).isEmpty)
        #expect(AccountSignInRules.actions(.password, nil).isEmpty)
    }

    @Test func aSignedInAccountSignsInAgainAndMaySwitchToAPassword() {
        let both = AccountSignInRules.actions(.provider, options(signIn: true, password: true))
        #expect(both == [.signInAgain, .toPassword])
        #expect(AccountSignInRules.offersSwitch(both))

        let again = AccountSignInRules.actions(.provider, options(signIn: true, password: false))
        #expect(again == [.signInAgain])
        // Signing in again is not a switch, so the section does not explain one.
        #expect(!AccountSignInRules.offersSwitch(again))

        #expect(
            AccountSignInRules.actions(.provider, options(signIn: false, password: true))
                == [.toPassword])
    }

    @Test func aPasswordAccountSwitchesToItsProviderWhereTheServerTakesIt() {
        let named = AccountSignInRules.actions(
            .password, options(signIn: true, label: "Fastmail", password: true))
        #expect(named == [.toProvider(label: "Fastmail")])
        #expect(AccountSignInRules.offersSwitch(named))
        #expect(
            AccountSettingsLabels.signInAction(named[0])
                == L10n.settings_account_switch_to_provider_named(provider: "Fastmail"))
        #expect(
            AccountSettingsLabels.signInAction(.toProvider(label: nil))
                == L10n.settings_account_switch_to_provider())

        // A password account already signs in with a password: nothing to switch to.
        #expect(
            AccountSignInRules.actions(.password, options(signIn: false, password: true)).isEmpty)
    }

    @Test func eachControlSaysHowItEnded() {
        #expect(AccountSettingsLabels.signInDone(.signInAgain) == L10n.settings_account_signed_in())
        #expect(
            AccountSettingsLabels.signInDone(.toProvider(label: "Fastmail"))
                == L10n.settings_account_switched_to_provider())
        #expect(
            AccountSettingsLabels.signInDone(.toPassword)
                == L10n.settings_account_switched_to_password())
        #expect(
            AccountSettingsLabels.signInFailed(.signInAgain) == L10n.settings_account_signin_failed())
        #expect(
            AccountSettingsLabels.signInFailed(.toPassword) == L10n.settings_account_switch_failed())
        #expect(
            AccountSettingsLabels.signInFailed(.toProvider(label: nil))
                == L10n.settings_account_switch_failed())
        #expect(
            AccountSettingsLabels.signInMethod(.password)
                == L10n.settings_account_signs_in_password())
        #expect(
            AccountSettingsLabels.signInMethod(.provider)
                == L10n.settings_account_signs_in_provider())
    }

    @Test func onlyStandardsAndJmapAccountsDrawTheSwitch() {
        #expect(AccountSignInRules.switchesSignIn(.imap))
        #expect(AccountSignInRules.switchesSignIn(.jmap))
        for kind in [AccountKind.dav, .microsoft, .google] {
            #expect(!AccountSignInRules.switchesSignIn(kind))
        }
    }

    @Test func theRedirectIsTheOneSetupUsesForTheKind() {
        #expect(AccountSignInRules.redirectURI(.imap) == ImapOAuthConfig.redirectURI)
        #expect(AccountSignInRules.redirectURI(.jmap) == JmapOAuthConfig.redirectURI)
        #expect(AccountSignInRules.redirectURI(.dav) == nil)
    }

    @Test func anImapSignInIsSignedInAgainFromTheBanner() {
        #expect(AccountSignInRules.remedy(.imapOauth) == .accountSignIn)
        #expect(AccountSignInRules.remedy(.jmapOauth) == .jmapReauth)
        #expect(AccountSignInRules.remedy(.microsoft) == .consent)
        #expect(AccountSignInRules.remedy(.google) == .consent)
        #expect(AccountSignInRules.remedy(.password) == .settings)
        #expect(AccountSignInRules.remedy(.jmap) == .settings)
        #expect(AccountSignInRules.remedy(nil) == .settings)
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

@Suite @MainActor struct SignInNameTests {
    @Test func aPromptToSignInAgainNamesTheAccountApartFromItsNamesake() {
        let model = MailboxModel()
        model.accounts = [
            AccountRow(id: "alice-imap", email: "alice@example.org", name: "", expanded: true),
            AccountRow(id: "bob-imap", email: "bob@example.org", name: "", expanded: true),
        ]
        model.accountsSnapshot = AccountsSnapshot(accounts: [
            AccountEntry(
                id: "alice-imap", address: "alice@example.org", kind: .imap, signIn: .password,
                uses: [AccountUse(capability: .mail, state: .on)],
                links: AccountLinksView(calendar: nil, contacts: nil, mail: nil), linkedFrom: [],
                linkCandidates: LinkCandidates(calendar: [], contacts: [], mail: [], suggested: []),
                endpoints: nil, distinction: .server(host: "mail.example.org")),
        ])
        #expect(
            model.signInName("alice-imap")
                == L10n.settings_account_distinguished(
                    address: "alice@example.org", detail: "mail.example.org"))
        // Not in the accounts snapshot: its address alone, and its id for one removed mid-pass.
        #expect(model.signInName("bob-imap") == "bob@example.org")
        #expect(model.signInName("gone") == "gone")
    }
}
