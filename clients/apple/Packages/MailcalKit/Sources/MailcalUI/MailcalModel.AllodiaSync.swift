// Keeping this device's mail-account list in step with the person's other devices. The core does
// the deciding and the writing; what is here is when to ask it, and what to do with the part it
// cannot answer alone.
//
// The pass BLOCKS on the network, so it runs off the main actor, like every other core call that
// reaches a server.

import Foundation
import MailcalBindings

/// What the Accounts settings screen draws about the person's other devices.
///
/// `report` is nil until a pass has run, which is not the same as a pass that found nothing: the
/// first has no business drawing a heading, and the second has earned drawing none.
struct AllodiaSyncState {
    var checking = false
    var report: AllodiaSyncReport?
    var failure: String?
    /// What the core knows about the sign-in itself, which is what a failure is DRAWN from.
    ///
    /// `failure` says a pass did not finish; this says whether that is the person's business and
    /// what they can do about it. Putting the failure's own text on screen is how a generated
    /// OAuth field name became product copy.
    var health: AllodiaGrantHealth = .ok

    /// Whether there is anything at all to put on screen.
    var hasSomethingToSay: Bool {
        guard let report else { return false }
        return !report.offers.isEmpty
            || !report.changedElsewhere.isEmpty
            || !report.removedElsewhere.isEmpty
    }
}

extension MailboxModel {
    /// Hand the core somewhere to remember what it has synced.
    ///
    /// Called from `connect`, before anything can ask for a pass. Unlike the Keychain writer it is
    /// not racing a dial: nothing syncs until somebody asks.
    func installAllodiaSyncStore(_ app: MailcalApp) {
        do {
            try app.useAllodiaSyncStateStore(store: UserDefaultsSyncStateStore())
        } catch {
            // The blob could not be read. Syncing is off for this launch rather than starting from
            // nothing, which would re-adopt every record and re-offer every account.
            logAppleLifecycle("allodia: the sync state could not be read (\(error)); not syncing")
        }
    }

    /// Run one pass, if there is any point in running one.
    ///
    /// Nobody signed in, or a launch on canned harness accounts: there is nothing worth syncing,
    /// and asking would only produce an error to draw. A pass already running is left to finish:
    /// two at once would race each other's writes.
    func syncAllodiaAccounts() async {
        guard let app, !allodiaSync.checking, !DevNamespace.usesCannedAccounts else { return }
        guard currentAllodiaAccount() != nil else { return }
        allodiaSync.checking = true
        allodiaSync.failure = nil
        do {
            let report = try await Task.detached(priority: .utility) {
                try app.syncAllodiaAccounts()
            }.value
            logAppleLifecycle(
                "allodia: sync done, \(report.sent) sent, \(report.offers.count) offered, "
                    + "\(report.changedElsewhere.count) changed elsewhere, "
                    + "\(report.removedElsewhere.count) removed elsewhere"
            )
            allodiaSync = AllodiaSyncState(report: report, health: app.allodiaGrantHealth())
        } catch {
            logAppleLifecycle("allodia: the sync pass did not finish (\(error))")
            allodiaSync.checking = false
            allodiaSync.failure = "\(error)"
            // The core's typed answer, not this error's text: it is what the screen draws, and
            // it stays as it was when nothing was learned about the sign-in.
            allodiaSync.health = app.allodiaGrantHealth()
        }
    }

    /// Move one account to a sync position.
    ///
    /// The core does everything the position takes, including reaching the service, so this
    /// runs off the main actor. The rows asking about an account changed or removed elsewhere go
    /// as soon as it is answered: a question still on screen afterwards reads as the answer not
    /// having worked.
    ///
    /// The position is re-read from the core rather than assumed, so a change the service refused
    /// leaves the control where it was instead of lying about what happened.
    func setAllodiaAccountSyncMode(_ accountId: String, _ mode: AllodiaAccountSyncMode) async {
        guard let app else { return }
        do {
            try await Task.detached(priority: .userInitiated) {
                try app.setAllodiaAccountSyncMode(accountId: accountId, mode: mode)
            }.value
            allodiaSync.failure = nil
            if let report = allodiaSync.report {
                allodiaSync.report = AllodiaSyncReport(
                    offers: report.offers,
                    changedElsewhere: report.changedElsewhere.filter { $0.accountId != accountId },
                    removedElsewhere: report.removedElsewhere.filter { $0.accountId != accountId },
                    sent: report.sent
                )
            }
        } catch {
            logAppleLifecycle("allodia: the account's sync position could not be set (\(error))")
            allodiaSync.failure = "\(error)"
        }
        readAccountsSynced()
    }

    /// How each account is shared. A local read per account; it never asks the service.
    ///
    /// The account list comes from the **core**, never from `syncSettings`: that cache is filled
    /// later in `connect` than this runs, so reading it here produced an empty map, and an empty
    /// map draws no control at all, a whole feature missing, with every build green.
    ///
    /// Empty in a build with no Allodia registration, which is what draws none of it.
    func readAccountsSynced() {
        guard let app, allodiaSignInAvailable() else {
            accountsSyncMode = [:]
            return
        }
        accountsSyncMode = Dictionary(
            uniqueKeysWithValues: app.syncSettings().accounts.map { account in
                (account.accountId, app.allodiaAccountSyncMode(accountId: account.accountId))
            }
        )
    }

    /// Every route that adds a mail account ends here: the manual form, the Microsoft and Google
    /// sign-ins, and the JMAP one (whose two routes converge before this).
    ///
    /// The pass belongs to all of them rather than to the form. An account added without one
    /// stays on this device until the next launch, and its card in Settings draws no sharing
    /// control at all, the mode map is read in the same call.
    func accountWasAdded() {
        setupError = nil
        needsSetup = false
        addingAccount = false
        syncAfterAccountChange()
    }

    /// The account list changed, so the person's other devices should hear about it now rather
    /// than at the next launch. A no-op when nobody is signed in.
    func syncAfterAccountChange() {
        readAccountsSynced()
        Task { await syncAllodiaAccounts() }
    }
}
