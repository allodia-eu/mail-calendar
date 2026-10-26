// How a standards or JMAP account signs in, and the switch to the other way in where its server
// takes it (docs/accounts.md rules 15 and 17), decided from the snapshot entry and the core's
// answer alone so each rule is tested without a view. The words are the catalog's.

import MailcalBindings

/// One control of an account's sign-in section.
enum SignInAction: Hashable {
    /// Signs an account that signs in through its provider in again, from its stored grant.
    case signInAgain
    /// Switches a password account to signing in through its provider, named when this build
    /// knows the provider's name.
    case toProvider(label: String?)
    /// Switches an account that signs in through its provider to a password.
    case toPassword

    /// Whether it runs in the browser.
    var opensBrowser: Bool { self != .toPassword }
}

/// What the expired-sign-in banner does for an account.
enum SignInRemedy: Equatable {
    /// A Microsoft or Google account signs in again through its own consent.
    case consent
    /// A JMAP account signed in through its provider re-authorises its stored grant.
    case jmapReauth
    /// An IMAP account signed in through its provider signs in again from its stored grant.
    case accountSignIn
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

    /// The offer a failed add makes; `nil` for every failure but an account already set up.
    init?(refusal error: Error, configToml: String, unreached: [AccountCapability] = []) {
        guard case let MailcalError.AlreadySetUp(accountId) = error else { return nil }
        self.accountId = accountId
        self.configToml = configToml
        self.unreached = unreached
    }
}

enum AccountSignInRules {
    /// Whether the account's page draws the sign-in section that can switch: a standards or JMAP
    /// account. Microsoft and Google sign in again through their own section, and an account used
    /// for no mail signs in to no mailbox.
    static func switchesSignIn(_ kind: AccountKind) -> Bool {
        kind == .imap || kind == .jmap
    }

    /// The controls the section offers once the core has said which ways in the server takes;
    /// none while it has not answered.
    static func actions(_ signIn: SignInMethod, _ options: AccountSignInOptions?) -> [SignInAction] {
        guard let options else { return [] }
        switch signIn {
        case .provider:
            return (options.signIn ? [.signInAgain] : []) + (options.password ? [.toPassword] : [])
        case .password:
            return options.signIn ? [.toProvider(label: options.providerLabel)] : []
        }
    }

    /// Whether `actions` switches the account to the other way in, which the section explains.
    static func offersSwitch(_ actions: [SignInAction]) -> Bool {
        actions.contains { $0 != .signInAgain }
    }

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

    /// The banner's remedy for an expired sign-in. An account whose family the core does not
    /// know is sent to Settings.
    static func remedy(_ provider: AccountProvider?) -> SignInRemedy {
        switch provider {
        case .microsoft, .google: .consent
        case .jmapOauth: .jmapReauth
        case .imapOauth: .accountSignIn
        case .password, .jmap, nil: .settings
        }
    }
}

extension AccountSettingsLabels {
    /// The section's status line.
    static func signInMethod(_ signIn: SignInMethod) -> String {
        switch signIn {
        case .password: L10n.settings_account_signs_in_password()
        case .provider: L10n.settings_account_signs_in_provider()
        }
    }

    /// A control's label.
    static func signInAction(_ action: SignInAction) -> String {
        switch action {
        case .signInAgain: L10n.signin_expired_action()
        case let .toProvider(label?): L10n.settings_account_switch_to_provider_named(provider: label)
        case .toProvider(nil): L10n.settings_account_switch_to_provider()
        case .toPassword: L10n.settings_account_switch_to_password()
        }
    }

    /// The line a control ends with once it has worked.
    static func signInDone(_ action: SignInAction) -> String {
        switch action {
        case .signInAgain: L10n.settings_account_signed_in()
        case .toProvider: L10n.settings_account_switched_to_provider()
        case .toPassword: L10n.settings_account_switched_to_password()
        }
    }

    /// The title of the alert a control ends with when it has not.
    static func signInFailed(_ action: SignInAction) -> String {
        action == .signInAgain
            ? L10n.settings_account_signin_failed()
            : L10n.settings_account_switch_failed()
    }
}
