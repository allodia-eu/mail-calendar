// How an account signs in again (docs/accounts.md rules 15 and 17): the remedy an expired sign-in
// takes, where a browser sign-in returns to, and the words for the Sign-in section the core
// decides (`AccountPage.signIn`). The words are the catalog's.

import MailcalBindings

/// What the expired-sign-in banner does for an account.
enum SignInRemedy: Equatable {
    /// A Microsoft or Google account signs in again through its own consent.
    case consent
    /// A JMAP account signed in through its provider re-authorises its stored grant.
    case jmapReauth
    /// An IMAP account signed in through its provider signs in again from its stored grant.
    case accountSignIn
    /// A calendar and contacts account a Nextcloud sign-in made signs in with Nextcloud again.
    case nextcloud
    /// A password or a pasted secret is entered again on the account's page.
    case settings
}

/// What the "already set up" refusal keeps, to put the credential just given on the account
/// already there rather than throwing it away.
struct ExistingAccountUpdate: Equatable {
    let accountId: String
    /// The config the refused add was given. It carries the credential, so it is never logged.
    let configToml: String
    /// The calendar and contacts the sign-in that produced the config does not reach, which the
    /// update asks about first (docs/accounts.md rule 18).
    let unreached: [AccountCapability]
    /// The credential is the grant setup's sign-in with Nextcloud keeps, not `configToml`.
    let nextcloud: Bool

    init(accountId: String, configToml: String, unreached: [AccountCapability], nextcloud: Bool) {
        self.accountId = accountId
        self.configToml = configToml
        self.unreached = unreached
        self.nextcloud = nextcloud
    }

    /// The offer a failed add makes; `nil` for every failure but an account already set up.
    init?(refusal error: Error, configToml: String, unreached: [AccountCapability] = []) {
        guard case let MailcalError.AlreadySetUp(accountId) = error else { return nil }
        self.init(
            accountId: accountId, configToml: configToml, unreached: unreached, nextcloud: false)
    }
}

enum AccountSignInRules {
    /// Where the provider sends the browser back to, by the account's kind: the redirect setup
    /// uses for that kind. An account already signing in through its provider keeps the one its
    /// grant was registered with, whatever is passed.
    static func redirectURI(_ kind: AccountKind) -> String? {
        switch kind {
        case .imap: ImapOAuthConfig.redirectURI
        case .jmap: JmapOAuthConfig.redirectURI
        case .dav, .microsoft, .google: nil
        }
    }

    /// The banner's remedy for an expired sign-in. An account a Nextcloud sign-in made is asked
    /// first, because it reports the provider family of the password it holds. An account whose
    /// family the core does not know is sent to Settings.
    static func remedy(_ provider: AccountProvider?, nextcloud: Bool = false) -> SignInRemedy {
        if nextcloud { return .nextcloud }
        return switch provider {
        case .microsoft, .google: .consent
        case .jmapOauth: .jmapReauth
        case .imapOauth: .accountSignIn
        case .password, .jmap, nil: .settings
        }
    }
}

extension AccountSettingsLabels {
    /// The line under the section's heading.
    static func signInLine(_ line: AccountSignInLine, _ entry: AccountEntry) -> String {
        switch line {
        case .password: L10n.settings_account_signs_in_password()
        case .provider: L10n.settings_account_signs_in_provider()
        case .nextcloud: L10n.settings_account_signs_in_nextcloud()
        case .atProvider: L10n.settings_account_signin_description()
        case .needsPermission: L10n.settings_account_needs_permission()
        case .refused: L10n.signin_expired_prompt(accounts: name(entry))
        }
    }

    /// A route's label.
    static func signInRoute(_ route: AccountSignInRoute) -> String {
        switch route {
        case .signInAgain: L10n.signin_expired_action()
        case let .toProvider(label?): L10n.settings_account_switch_to_provider_named(provider: label)
        case .toProvider(nil): L10n.settings_account_switch_to_provider()
        case .toNextcloud: L10n.settings_account_switch_to_provider_named(provider: "Nextcloud")
        }
    }

    /// The line a route ends with once it has worked.
    static func signInDone(_ route: AccountSignInRoute) -> String {
        switch route {
        case .signInAgain: L10n.settings_account_signed_in()
        case .toProvider, .toNextcloud: L10n.settings_account_switched_to_provider()
        }
    }

    /// The title of the alert a route ends with when it has not.
    static func signInFailed(_ route: AccountSignInRoute) -> String {
        if case .signInAgain = route { return L10n.settings_account_signin_failed() }
        return L10n.settings_account_switch_failed()
    }
}
