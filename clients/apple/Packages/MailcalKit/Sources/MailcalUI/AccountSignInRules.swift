// How an account signs in again (docs/accounts.md rules 15 and 17): where a browser sign-in
// returns to, how this host waits for it, and the words for the Sign-in section the core decides
// (`AccountPage.signIn`). Which step a sign-in is at, and what it came to, are the core's
// (`BrowserSignIn`); the words are the catalog's.

import MailcalBindings

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

/// How this host waits for the browser a sign-in opens to come back.
enum SignInListener: Equatable {
    /// The platform's auth session, which catches a return to this scheme.
    case session(callbackScheme: String)
    /// The loopback port bound for this sign-in before it was asked for.
    case loopback
    /// Nothing comes back: the core asks the server whether the person has finished
    /// (docs/nextcloud-sign-in.md rule 5), so only the person closes the session.
    case server
}

/// Where the browser opens for a sign-in, and where it returns.
struct SignInOpening: Equatable {
    let url: String
    let redirect: String?
}

/// The question a sign-in asks before the browser opens (docs/accounts.md rule 18).
struct SignInQuestion: Equatable {
    let attempt: UInt64
    let set: UnreachedSet
}

enum AccountSignInRules {
    /// Where the provider sends the browser back to for an account of `kind`: the redirect setup
    /// uses for that kind. `google` is this build's registered Google redirect, `nil` where a
    /// loopback port is bound for each sign-in instead. Empty for a calendar and contacts
    /// account, whose sign-in returns nothing. An account already signing in through its provider
    /// returns where its grant was registered, which the core reads from the grant.
    static func redirectURI(_ kind: AccountKind, google: String?) -> String? {
        switch kind {
        case .microsoft: MicrosoftOAuthConfig.redirectURI
        case .google: google
        case .imap: ImapOAuthConfig.redirectURI
        case .jmap: JmapOAuthConfig.redirectURI
        case .dav: ""
        }
    }

    /// How to wait for a browser returning to `redirect`, given the loopback bound for the
    /// sign-in, if any. `nil` when nothing here can: a web address no loopback answers, or no
    /// address at all.
    static func listener(redirect: String?, loopback: String?) -> SignInListener? {
        guard let redirect else { return .server }
        if let loopback, redirect == loopback { return .loopback }
        guard let colon = redirect.firstIndex(of: ":") else { return nil }
        let scheme = String(redirect[..<colon])
        guard !scheme.isEmpty, !["http", "https"].contains(scheme.lowercased()) else { return nil }
        return .session(callbackScheme: scheme)
    }

    /// Whether the browser this host holds for `attempt` stays: while the core still holds that
    /// attempt, and once it is open, while the core still waits on it. A loopback bound before
    /// the core answered waits for its browser.
    static func keeps(_ record: BrowserSignIn?, attempt: UInt64, opened: Bool) -> Bool {
        guard let record, record.attempt == attempt else { return false }
        if case .inBrowser = record.stage { return true }
        return !opened
    }

    /// The browser to open now: the core waits on one, and none was opened for the attempt.
    static func opening(_ record: BrowserSignIn?, opened: Bool) -> SignInOpening? {
        guard !opened, case let .inBrowser(url, redirect)? = record?.stage else { return nil }
        return SignInOpening(url: url, redirect: redirect)
    }

    /// The question the sign-in asks before its browser opens, while it does.
    static func question(_ browser: BrowserSignIn?) -> SignInQuestion? {
        guard let browser, case let .asking(unreached) = browser.stage,
              let set = UnreachedSet(unreached)
        else { return nil }
        return SignInQuestion(attempt: browser.attempt, set: set)
    }

    /// What the core is handed for an answer to that question; `nil` cancels the sign-in.
    static func answer(_ answer: UnreachedAnswer) -> UnreachedUses? {
        guard case let .proceed(uses) = answer.decision else { return nil }
        return uses
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

    /// What the section says while the browser holds the account's sign-in, `name` being how the
    /// account is named; `nil` at every other step.
    static func signInWaiting(_ browser: BrowserSignIn?, name: String) -> String? {
        guard case let .inBrowser(_, redirect)? = browser?.stage else { return nil }
        return redirect == nil
            ? L10n.nextcloud_sign_in_waiting()
            : L10n.settings_account_signin_waiting(email: name)
    }
}
