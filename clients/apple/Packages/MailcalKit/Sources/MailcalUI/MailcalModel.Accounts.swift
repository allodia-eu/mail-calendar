// Settings → Accounts' traffic with the core: the one snapshot every account is drawn from, and the
// changes an account's page makes (docs/accounts.md rules 11 to 14). A use, a link and a removal
// are the core's to run (docs/waiting.md): dispatched, they answer at once and the page says they
// run. Every other change may touch the Keychain or dial a server, so it runs off the main actor and
// answers with what the page should say.

import Foundation
import MailcalBindings

/// The open account's page as the core decided it (`account_page`), read again on every Settings
/// signal and whenever `accountPage` changes, and what a change on it came to.
struct AccountPageState {
    /// Its uses, links and removal.
    var record: AccountPage?
    /// What a change on the page came to, until the page says it.
    var failure: AccountPageAlert?
    /// The last of the core's change notices said, so each is said once.
    var noticeSaid: UInt64 = 0
}

/// What switching a use on or off answered.
enum UseChangeOutcome: Equatable {
    /// Done; the page redraws from the next snapshot.
    case applied
    /// The provider has not granted it: sign the account in again, asking for it.
    case askProvider
    /// The account has no server for it.
    case needsEndpoint
    case failed(String)
}

/// What a person reads for a core error: its own message, without the enum around it. A refused
/// sign-in is said in the person's language, since the server's reply is meant for a developer
/// and the core has logged it, and an account already set up, a sign-in as another account or
/// from another request, and a use that did not connect likewise.
func coreMessage(_ error: Error) -> String {
    switch error {
    case MailcalError.SignInRefused(_):
        return L10n.signin_refused()
    case MailcalError.AlreadySetUp(_):
        return L10n.account_already_set_up()
    case MailcalError.DifferentAccount:
        return L10n.signin_different_account()
    case MailcalError.StaleSignIn:
        return L10n.signin_stale()
    case let MailcalError.UseNotConnected(capability, problem):
        return useNotConnectedMessage(capability, problem)
    case let MailcalError.Config(message), let MailcalError.Connect(message),
         let MailcalError.Engine(message), let MailcalError.Composer(message):
        return message
    case let MailcalError.CertificateRejected(reason, _):
        return reason
    default:
        return error.localizedDescription
    }
}

/// What a failed sign-in says when signing in again as it says settles it: the browser came back
/// from another request, or as another account. `nil` for any other failure, which each route
/// words its own way.
func signInRemedy(_ error: Error) -> String? {
    switch error {
    case MailcalError.DifferentAccount, MailcalError.StaleSignIn:
        return coreMessage(error)
    default:
        return nil
    }
}

/// Why a calendar or address book did not connect to a server of its own (docs/accounts.md rule
/// 18). The capability is never mail here, and reads as the calendar if it is.
func useNotConnectedMessage(_ capability: AccountCapability, _ problem: UseProblem) -> String {
    let contacts = capability == .contacts || capability == .colleagues
    switch problem {
    case .unreachable:
        return contacts
            ? L10n.separate_server_unreachable_contacts()
            : L10n.separate_server_unreachable_calendar()
    case .needsOwnSignIn:
        return contacts
            ? L10n.settings_account_use_own_sign_in_contacts()
            : L10n.settings_account_use_own_sign_in_calendar()
    case .signInRefused:
        return L10n.signin_refused()
    case .serverChanged:
        return contacts
            ? L10n.settings_account_use_server_changed_contacts()
            : L10n.settings_account_use_server_changed_calendar()
    case .noneOnServer:
        return contacts
            ? L10n.settings_account_use_none_on_server_contacts()
            : L10n.settings_account_use_none_on_server_calendar()
    }
}

extension MailboxModel {
    /// Pulls the snapshot again, and publishes it only when it changed, so a signal that changed
    /// nothing on the page leaves it, and whatever is being typed on it, alone. The last account
    /// gone returns the window to first-run setup, decided here rather than from the mailbox: an
    /// account without mail is in none of its rows (docs/accounts.md rule 11).
    func refreshAccountsSnapshot() {
        guard let app else { return }
        let fresh = app.accountsSnapshot()
        if fresh.accounts.isEmpty, !accountsSnapshot.accounts.isEmpty {
            logAppleLifecycle("accounts: the last account is gone; returning to first-run setup")
            needsSetup = true
        }
        if fresh != accountsSnapshot { accountsSnapshot = fresh }
        if let accountPage, !fresh.accounts.contains(where: { $0.id == accountPage }) {
            self.accountPage = nil
        }
        refreshAccountPage()
        refreshAllodiaDevices()
        refreshServerChanges()
        refreshSeparateServers()
        refreshSetupLinks()
        driveBrowserSignIn()
    }

    /// Reads the open account's page again, and says what a change came to, once: on its page,
    /// or for a removal, wherever the person is.
    func refreshAccountPage() {
        guard let app else { return }
        let fresh = accountPage.flatMap { app.accountPage(accountId: $0) }
        if fresh != accountPageState.record { accountPageState.record = fresh }
        if let raised = fresh?.notice, raised.id > accountPageState.noticeSaid {
            accountPageState.noticeSaid = raised.id
            logAppleLifecycle("accounts: an account's page has something to say")
            accountPageState.failure = AccountSettingsLabels.changeFailed(raised.notice)
        }
        if let raised = app.accountsNotice(), raised.id > accountPageState.noticeSaid {
            accountPageState.noticeSaid = raised.id
            logAppleLifecycle("accounts: a removed account's stored sign-in could not be erased")
            if case let .removeFailed(detail) = raised.notice {
                removalFailure = L10n.remove_account_failed(error: detail)
            }
        }
    }

    /// Asks the core for a change on an account's page; it answers at once, and the page says it
    /// runs.
    func changeAccount(_ intent: AccountPageIntent) {
        app?.dispatch(intent: .accountPage(intent: intent))
    }

    /// Account `id`'s page opened: its server is asked again which mailboxes its sign-in opens,
    /// on the core's own thread, and the page is signalled once it answers.
    func accountPageOpened(_ id: String) {
        app?.accountPageOpened(accountId: id)
    }

    func accountEntry(_ id: String) -> AccountEntry? {
        accountsSnapshot.accounts.first { $0.id == id }
    }

    /// Switches one of an account's uses on or off, for setup's own steps, which wait on the
    /// answer. Blocking in the core: switching a use off deletes what the device holds of it.
    func setAccountUse(
        _ id: String, _ capability: AccountCapability, on: Bool
    ) async -> UseChangeOutcome {
        guard let app else { return .failed(L10n.app_unavailable()) }
        do {
            let change = try await Task.detached(priority: .userInitiated) {
                try app.setAccountCapability(accountId: id, capability: capability, on: on)
            }.value
            switch change {
            case .applied: return .applied
            case .needsConsent: return .askProvider
            case .needsEndpoint: return .needsEndpoint
            }
        } catch {
            logAppleLifecycle("accounts: switching a use failed (\(coreMessage(error)))")
            return .failed(coreMessage(error))
        }
    }

    /// Links an account to another for one slot, or clears that link, for setup's link step, which
    /// stores its picks one after another. Answers why not, or `nil` once stored.
    func setAccountLink(_ id: String, _ slot: LinkSlot, _ target: String?) async -> String? {
        guard let app else { return L10n.app_unavailable() }
        do {
            // Stored through the Keychain, which may block.
            try await Task.detached(priority: .userInitiated) {
                try app.setAccountLink(accountId: id, slot: slot, target: target)
            }.value
            return nil
        } catch {
            logAppleLifecycle("accounts: linking an account failed (\(coreMessage(error)))")
            return coreMessage(error)
        }
    }

    /// What `endpoints` leaves out of account `id`'s servers, as its save would refuse it. Reads
    /// only what is stored, so the page asks it as the person types.
    func endpointsGap(_ id: String, _ endpoints: AccountEndpoints) -> AccountEndpointsGap? {
        app?.accountEndpointsGap(accountId: id, endpoints: endpoints)
    }

    /// Opens Settings → Accounts on `id`'s page, from a prompt in the main window that points at
    /// it. The shell watches `settingsRequest` and presents Settings.
    func openAccountPage(_ id: String) {
        accountPage = id
        settingsRequest = .accounts
    }
}
