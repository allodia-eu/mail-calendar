// Drives a showcase (screenshot) run to the screen `MAILCAL_SHOWCASE_SCREEN` names, so the store
// screenshot set is captured without a single tap, `scripts/dev/showcase.sh` relaunches the app once
// per screen per language. Inert unless `ShowcaseMode.isOn`, which is hard-`false` in a release build.

#if os(macOS)
import AppKit
#endif
import SwiftUI
import MailcalBindings

extension ContentView {
    /// Sizes the macOS window to a store-valid screenshot frame. `screencapture -l` captures the
    /// window frame (title bar included), and a Retina display doubles it, so 1440×900 points
    /// lands exactly on the Mac App Store's 2880×1800. Resizing the window from outside would need
    /// Accessibility permission; the app can just do it itself.
    ///
    /// Deferred a beat, and with the frame autosave detached: AppKit restores the developer's saved
    /// window frame *after* `onAppear`, so sizing it synchronously here would simply be overwritten.
    func showcaseSizeWindowIfNeeded() {
        #if os(macOS)
        guard ShowcaseMode.isOn else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            guard let window = NSApplication.shared.windows.first(where: \.isVisible) else { return }
            _ = window.setFrameAutosaveName("")
            var frame = window.frame
            frame.size = NSSize(width: 1440, height: 900)
            window.setFrame(frame, display: true)
            window.center()
            logAppleLifecycle("showcase: window sized to \(Int(frame.width))x\(Int(frame.height)) points")
        }
        #endif
    }
    // `hasReadingPane`, whether a message shows in a persistent pane beside the list, lives in
    // Mailcal.AutoAdvance.swift now, because archive/delete needs the same question answered. The
    // "list" screenshot opens the first message into that pane where one exists; on iPhone, opening
    // it would push the list off-screen, so the list stands alone.

    /// Drives to the requested showcase screen once the mailbox rows have loaded. Called from
    /// `onAppear` and on every row-count change, and one-shot-guarded, so it fires exactly once.
    func showcaseDriveIfNeeded() {
        guard ShowcaseMode.isOn, !didShowcaseDrive else { return }
        switch ShowcaseMode.screen {
        case .settings:
            didShowcaseDrive = true
            settingsCategory = ShowcaseMode.settingsCategory ?? .general
        case .addAccount, .setupEmail, .setupDetected, .setupUntrusted, .setupManual:
            // One arm for all five: opening the sheet is the whole drive. Which step the
            // documentation screens land on is decided inside AccountSetupDetectView, from
            // `ShowcaseMode.setupSeed` and the core's scripted detection, so this never has to
            // know, and can never disagree with, what the app would really show.
            didShowcaseDrive = true
            model.setupError = nil
            model.addingAccount = true
        case .mcpOff, .mcpOn, .mcpAccounts, .mcpSend:
            // Agent access lives under Settings → Advanced, which `ShowcaseMode.settingsCategory`
            // preselects. Here we only set the *state* each guide step pictures, in the same
            // order a reader turns them on: the server, then a mailbox, then direct send. Each
                // step is cumulative, because that is how the panel actually looks by then.
            didShowcaseDrive = true
            applyShowcaseMcpState()
            settingsCategory = ShowcaseMode.settingsCategory ?? .general
        case .list:
            // Needs a loaded row to open; leave the guard down until one arrives.
            guard hasReadingPane, let first = model.rows.first, case let .flat(message) = first else {
                return
            }
            didShowcaseDrive = true
            open(message)
        case .reply:
            let target = showcaseReply(locale: ShowcaseMode.seedLocale)
            guard openShowcaseMessage(account: target.account, key: target.messageKey) else { return }
            didShowcaseDrive = true
        case .invitation:
            // Opening the message is the whole drive: the invitation card is part of the reading
            // view, and the core has already primed the calendar, so the card comes up with its
            // day preview expanded rather than "we haven't looked at your calendar".
            let target = showcaseInvitation()
            guard openShowcaseMessage(account: target.account, key: target.messageKey) else { return }
            didShowcaseDrive = true
        case .calendar:
            // The calendar needs no mail rows, it opens on today's week and pulls its own page, so
            // this fires on the first `onAppear` without waiting for the list to load. `showCalendar`
            // sets the scene state and dispatches the calendar sync, exactly as the sidebar's
            // Calendar row does, so the screenshot shows the calendar the user gets.
            didShowcaseDrive = true
            showCalendar()
        }
    }

    /// Puts the agent (MCP) settings into the state one documentation screen pictures.
    ///
    /// Cumulative on purpose, `mcp-send` shows the panel as it really looks once a reader has
    /// worked through the guide, with the server on and a mailbox granted above it, rather than
    /// a direct-send toggle floating over an otherwise-off panel that no user would ever see.
    ///
    /// Only the *first* showcase account is granted, never both: the guide's point is that
    /// turning the server on and granting a mailbox are two separate decisions, and a screenshot
    /// with every account already ticked would quietly contradict it.
    private func applyShowcaseMcpState() {
        let screen = ShowcaseMode.screen
        guard screen != .mcpOff else { return }
        model.setMcpEnabled(true)
        guard screen != .mcpOn else { return }
        // Read off `mcpSettings`, which is the list the panel itself renders, so the account
        // this ticks is the row that appears ticked, whatever order the mailbox list is in.
        if let first = model.mcpSettings?.accounts.first {
            model.setMcpAccountExposed(first.accountId, true)
        }
        guard screen != .mcpAccounts else { return }
        model.setMcpAllowDirectSend(true)
    }

    /// Opens the reply composer once the target message's body has loaded, the quoted original (and
    /// the sample reply text seeded above it) is only available then. Fires once per launch.
    func showcaseReplyIfNeeded(readingKey: String?) {
        guard ShowcaseMode.isOn, ShowcaseMode.screen == .reply, !didShowcaseReply,
              compose == nil, readingKey != nil, let opened = openedMessage
        else { return }
        didShowcaseReply = true
        beginReply(opened.account, opened.key, all: false)
    }

    /// Opens the showcase's designated message. It is a standalone (unthreaded) message in both
    /// locale seeds, so it always lists as a flat row. Returns `false` until that row has loaded.
    private func openShowcaseMessage(account: String, key: String) -> Bool {
        for case .flat(let message) in model.rows
        where message.account == account && message.key == key {
            open(message)
            return true
        }
        return false
    }
}
