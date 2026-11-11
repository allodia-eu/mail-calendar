// Keeping this device's account list in step with the person's other devices. The core runs the
// pass and every change (docs/settings.md category 10); what is here is handing it somewhere to
// keep its bookkeeping, and reading what it says.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Hand the core somewhere to remember what it has synced, which starts a pass when someone is
    /// signed in.
    ///
    /// Called from `connect`. A launch on canned harness accounts installs none: those accounts
    /// must never reach the person's other devices, and with no store the core runs no pass.
    func installAllodiaSyncStore(_ app: MailcalApp) {
        guard !DevNamespace.usesCannedAccounts else { return }
        do {
            try app.useAllodiaSyncStateStore(store: UserDefaultsSyncStateStore())
        } catch {
            // The blob could not be read. Syncing is off for this launch rather than starting from
            // nothing, which would re-adopt every record and re-offer every account.
            logAppleLifecycle("allodia: the sync state could not be read (\(error)); not syncing")
        }
    }

    /// Reads what the other devices have to say again, publishing it only when it changed. Called
    /// on an OtherDevices signal, which the core raises when a pass starts and when it ends, on
    /// every AccountPage signal, which a Keep raises, and on every Settings signal, which a
    /// sign-in or sign-out raises.
    func refreshAllodiaDevices() {
        let fresh = app?.allodiaDevices()
        if fresh != allodiaDevices { allodiaDevices = fresh }
    }
}
