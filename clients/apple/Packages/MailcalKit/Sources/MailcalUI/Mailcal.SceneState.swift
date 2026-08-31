import Foundation
import MailcalBindings
import SwiftUI
#if os(iOS)
import UIKit
#endif

extension ContentView {
    /// Shuts the iPhone's sidebar drawer. A no-op everywhere else, no other Apple surface has one.
    ///
    /// Called from every navigation below rather than watched for from the drawer: tapping the
    /// folder you are already in changes nothing to observe, and a drawer that stays open on that
    /// tap looks like the tap missed.
    func dismissSidebar() {
        #if os(iOS)
        withAnimation { sidebarOpen = false }
        #endif
    }

    func selectAccount(_ id: String?) {
        dismissSidebar()
        sceneDestination = AppDestination.mail.rawValue
        sceneSelectedAccount = id ?? ""
        sceneSelectedFolder = ""
        clearOpenedMessage()
        model.selectAccount(id)
    }

    /// Opens a folder, in the account whose tree the pane row sits under, which is not
    /// necessarily the selected one.
    ///
    /// One intent carrying both halves. Every account's tree is on screen at once and a folder key
    /// is unique only within its account, so the key alone would be resolved against whichever
    /// account happened to be selected, or, from All Inboxes, against none, leaving the list
    /// exactly as it was (`docs/folder-pane.md`, rule 14). An account's own all-mail view is
    /// `selectAccount`.
    func selectFolder(in account: String, key: String) {
        dismissSidebar()
        sceneDestination = AppDestination.mail.rawValue
        sceneSelectedAccount = account
        sceneSelectedFolder = key
        clearOpenedMessage()
        model.selectFolder(account: account, folder: key)
    }

    /// Back to the mailbox, with the account and folder exactly as they were left.
    ///
    /// Deliberately not `selectAccount(model.selectedAccount)`: dispatching an account resets the
    /// selected folder in the core, so re-selecting would drop the user into All Mail every time
    /// they came back from the calendar.
    func showMail() {
        dismissSidebar()
        sceneDestination = AppDestination.mail.rawValue
        model.destination = .mail
    }

    func showCalendar() {
        dismissSidebar()
        sceneDestination = AppDestination.calendar.rawValue
        clearOpenedMessage()
        model.showCalendar()
    }

    func showContacts() {
        dismissSidebar()
        sceneDestination = AppDestination.contacts.rawValue
        clearOpenedMessage()
        openedContact = nil
        model.showContacts()
    }

    func setViewMode(_ mode: ViewMode) {
        sceneViewMode = viewModeToken(mode)
        model.setMode(mode)
    }

    func removeAccount(_ id: String) {
        if sceneSelectedAccount == id || model.selectedAccount == id {
            sceneSelectedAccount = ""
            sceneSelectedFolder = ""
            clearOpenedMessage()
        }
        model.removeAccount(id)
    }

    func setOpenedMessage(_ message: OpenedMessage) {
        openedMessage = message
        sceneOpenedAccount = message.account
        sceneOpenedKey = message.key
    }

    func clearOpenedMessage() {
        openedMessage = nil
        sceneOpenedAccount = ""
        sceneOpenedKey = ""
    }

    func handleScenePhaseChange(_ phase: ScenePhase) {
        switch phase {
        case .active:
            logAppleLifecycle("scene active")
            restoreSceneIfPossible()
            if hasActivatedScene {
                refreshActiveScene()
            }
            hasActivatedScene = true
        case .background:
            logAppleLifecycle("scene background")
            captureVisibleSceneState()
            #if os(iOS)
            // Entering the background is the moment to line up the next background mail sync
            // (docs/background-sync.md); iOS decides when to actually run it.
            scheduleBackgroundRefresh()
            #endif
        case .inactive:
            logAppleLifecycle("scene inactive")
        @unknown default:
            break
        }
    }

    #if os(iOS)
    /// Asks for notification permission, but only once **both** questions ahead of it are settled:
    /// the user has an account (asking on the empty setup screen is premature), and the
    /// usage-statistics question has been answered. Two prompts must never stack, and the system
    /// alert is the one we cannot dismiss or reposition, so it goes last.
    ///
    /// Called from every edge that can settle either condition; `requestAuthorization` is
    /// idempotent, so being called more than once per launch costs nothing.
    ///
    /// A showcase run never asks: the alert would land on top of the screenshot being taken.
    func requestNotificationsIfSettled() {
        guard !ShowcaseMode.isOn, !model.needsSetup, model.analyticsConsent?.asked == true else {
            return
        }
        MailNotifier.requestAuthorization()
    }
    #endif

    func applePlatformSummary() -> String {
        #if os(iOS)
        let device = UIDevice.current
        return "\(device.model), \(device.systemName) \(device.systemVersion)"
        #else
        return ProcessInfo.processInfo.operatingSystemVersionString
        #endif
    }

    func restoreSceneIfPossible() {
        guard !sceneRestorationComplete, model.app != nil, !model.needsSetup else { return }

        if let desiredMode = viewMode(from: sceneViewMode),
           viewModeToken(model.mode) != sceneViewMode {
            model.setMode(desiredMode)
            return
        }

        // A stored destination other than mail restores in one step: both the calendar and
        // contacts open on their own data and need nothing from the mail selection below.
        switch AppDestination(rawValue: sceneDestination) {
        case .calendar:
            if model.destination != .calendar { model.showCalendar() }
            sceneRestorationComplete = true
            return
        case .contacts:
            if model.destination != .contacts { model.showContacts() }
            sceneRestorationComplete = true
            return
        case .mail, nil:
            break
        }

        // Mail was the stored destination but the model is elsewhere (a launch default, or a
        // showcase/verification flag): put it back on mail before restoring the selection.
        if model.destination != .mail {
            model.selectAccount(sceneSelectedAccount.isEmpty ? nil : sceneSelectedAccount)
            return
        }

        if !sceneSelectedAccount.isEmpty {
            guard model.accounts.contains(where: { $0.id == sceneSelectedAccount }) else {
                if model.accounts.isEmpty { return }
                sceneRestorationComplete = true
                return
            }
            if model.selectedAccount != sceneSelectedAccount {
                model.selectAccount(sceneSelectedAccount)
                return
            }
        }

        // A folder is restored only alongside its account: a folder key means nothing on its own
        // (`docs/folder-pane.md`, rule 14), and a scene saved by an older build can hold one
        // without an account.
        if !sceneSelectedFolder.isEmpty, !sceneSelectedAccount.isEmpty {
            guard model.folders.contains(where: { $0.key == sceneSelectedFolder }) else {
                if model.folders.isEmpty { return }
                sceneRestorationComplete = true
                return
            }
            if model.selected != sceneSelectedFolder {
                // The account block above has already run to completion, so this restores the
                // folder into the account it was left in.
                model.selectFolder(account: sceneSelectedAccount, folder: sceneSelectedFolder)
                return
            }
        }

        if !sceneOpenedAccount.isEmpty, !sceneOpenedKey.isEmpty, openedMessage == nil {
            guard !model.rows.isEmpty else { return }
            restoreOpenedMessageIfVisible()
        }

        sceneRestorationComplete = true
    }

    private func restoreOpenedMessageIfVisible() {
        for row in model.rows {
            switch row {
            case .flat(let message)
                where message.account == sceneOpenedAccount && message.key == sceneOpenedKey:
                open(message)
                return
            case .thread(let thread):
                if let message = thread.messages.first(where: {
                    $0.account == sceneOpenedAccount && $0.key == sceneOpenedKey
                }) {
                    expandedThreads.insert(threadKey(thread))
                    openThreadMessage(thread, message)
                    return
                }
            default:
                continue
            }
        }
    }

    private func captureVisibleSceneState() {
        sceneDestination = model.destination.rawValue
        sceneSelectedAccount = model.selectedAccount ?? ""
        sceneSelectedFolder = model.selected ?? ""
        sceneViewMode = viewModeToken(model.mode)
        if let openedMessage {
            sceneOpenedAccount = openedMessage.account
            sceneOpenedKey = openedMessage.key
        } else {
            sceneOpenedAccount = ""
            sceneOpenedKey = ""
        }
    }

    private func refreshActiveScene() {
        guard model.app != nil, !model.needsSetup else { return }
        switch model.destination {
        case .calendar: model.showCalendar()
        // Re-syncs the address books rather than the mailbox, coming back to a Contacts window
        // and being handed stale mail counts is not what the visible surface asked for. The
        // search field is view state and survives, so this deliberately does not clear the query
        // the way entering the tab does.
        case .contacts: model.app?.dispatch(intent: .refreshContacts)
        case .mail: model.refresh()
        }
    }

    private func viewMode(from token: String) -> ViewMode? {
        switch token {
        case "flat": return .flat
        case "threaded": return .threaded
        default: return nil
        }
    }

    private func viewModeToken(_ mode: ViewMode) -> String {
        switch mode {
        case .flat: return "flat"
        case .threaded: return "threaded"
        }
    }
}
