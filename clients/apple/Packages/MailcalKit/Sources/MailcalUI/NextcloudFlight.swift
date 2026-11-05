// The browser half of a sign-in with Nextcloud, which every surface offering it shares
// (docs/nextcloud-sign-in.md rules 5 and 6). The core starts the sign-in, the login page opens in
// an in-app browser tab, and the core is asked whether the person has finished: every poll
// interval, at once when the app is active again, and once more when the person closes the tab.
// There is no redirect back, so the poll is all the waiting there is.
//
// What a surface does with the grant is its own (`NextcloudSignIn`). What they share is that the
// server hands the grant out once, so one arriving for an attempt nobody waits on is withdrawn.

#if os(macOS)
import AppKit
#else
import UIKit
#endif
import AuthenticationServices
import Foundation
import MailcalBindings

/// A granted sign-in, held until it becomes an account. Opaque, and it carries the app password,
/// so it is never printed.
struct NextcloudGrant: Equatable, CustomStringConvertible, CustomDebugStringConvertible {
    let value: String

    var description: String { "NextcloudGrant(..)" }
    var debugDescription: String { description }
}

/// How a flight ended.
enum NextcloudFlightEnd: Equatable {
    case granted(NextcloudGrant)
    /// The server let the sign-in go, twenty minutes after it started.
    case expired
    /// Why the sign-in did not start, or ended without a grant.
    case failed(String)
    /// Retired: the person cancelled, closed the browser without granting, or left the surface.
    case cancelled

    /// What a surface says about an end without a grant; `nil` for one that says nothing.
    var said: String? {
        switch self {
        case .granted, .cancelled: nil
        case .expired: L10n.account_signin_timeout()
        case let .failed(said): said
        }
    }

    /// What a refusal to start or to poll says: a failed connect reads as the sign-in failing,
    /// since the server's own words are for a developer.
    static func failure(_ error: Error) -> NextcloudFlightEnd {
        if case MailcalError.Connect(_) = error { return .failed(L10n.setup_imap_signin_failed()) }
        return .failed(coreMessage(error))
    }
}

/// What ends a pause between two polls.
enum NextcloudWakeSignal: Equatable {
    /// The interval passed.
    case tick
    /// The app is active again, which is when somebody who granted access elsewhere is back.
    case returned
    /// The person closed the browser.
    case closed
    /// The browser could not show the page.
    case browserFailed
    /// The attempt was retired.
    case cancelled
}

/// The pause between polls, ended by the interval or by whichever signal comes first. A closed
/// or failed browser, or a retirement, arriving while a poll runs is kept for the next pause; a
/// return then is not, since the poll it asks for is already running.
@MainActor
final class NextcloudWake {
    private var waiting: CheckedContinuation<NextcloudWakeSignal, Never>?
    private var timer: Task<Void, Never>?
    private var pending: NextcloudWakeSignal?

    func pause(for interval: Duration) async -> NextcloudWakeSignal {
        if let pending {
            self.pending = nil
            return pending
        }
        if Task.isCancelled { return .cancelled }
        return await withTaskCancellationHandler {
            await withCheckedContinuation { continuation in
                waiting = continuation
                timer = Task { [weak self] in
                    try? await Task.sleep(for: interval)
                    if !Task.isCancelled { self?.fire(.tick) }
                }
            }
        } onCancel: {
            Task { @MainActor in self.fire(.cancelled) }
        }
    }

    func fire(_ signal: NextcloudWakeSignal) {
        if let waiting {
            self.waiting = nil
            timer?.cancel()
            timer = nil
            waiting.resume(returning: signal)
        } else if signal != .tick, signal != .returned, pending != .browserFailed {
            pending = signal
        }
    }
}

/// Polls until the sign-in ends. `poll` asks the core once; `pause` waits for the next reason to
/// ask. A browser the person closed gets one more poll, because they may have granted access the
/// moment before, and then the attempt retires.
@MainActor
func awaitNextcloudGrant(
    poll: () async -> Result<NextcloudSignInPoll, Error>,
    pause: () async -> NextcloudWakeSignal
) async -> NextcloudFlightEnd {
    var closed = false
    while true {
        if Task.isCancelled { return .cancelled }
        switch await poll() {
        case .success(.waiting): break
        case .success(.expired): return .expired
        case let .success(.granted(grant)): return .granted(NextcloudGrant(value: grant))
        case let .failure(error): return .failure(error)
        }
        if closed { return .cancelled }
        switch await pause() {
        case .tick, .returned: continue
        case .closed: closed = true
        case .browserFailed: return .failed(L10n.account_signin_browser_failed())
        case .cancelled: return .cancelled
        }
    }
}

/// Flies one sign-in: `begin` is the surface's own start, run off the main actor. A grant that
/// arrives once the calling task was cancelled is withdrawn here, and the flight ends
/// `.cancelled`.
@MainActor
func flyNextcloudSignIn(
    _ app: MailcalApp,
    begin: @escaping @Sendable (MailcalApp) throws -> NextcloudSignInStart
) async -> NextcloudFlightEnd {
    let start: NextcloudSignInStart
    do {
        start = try await Task.detached(priority: .userInitiated) { try begin(app) }.value
    } catch {
        logBrowserSignIn("nextcloud", "the sign-in did not start (\(coreMessage(error)))")
        return .failure(error)
    }
    if Task.isCancelled { return .cancelled }
    let wake = NextcloudWake()
    let browser = NextcloudBrowser()
    guard browser.open(start.loginUrl, ended: { wake.fire($0) }) else {
        return .failed(L10n.account_signin_browser_failed())
    }
    let returned = NotificationCenter.default.addObserver(
        forName: NextcloudBrowser.activated, object: nil, queue: .main
    ) { _ in
        MainActor.assumeIsolated { wake.fire(.returned) }
    }
    let interval = Duration.milliseconds(Int64(clamping: nextcloudPollIntervalMs()))
    let pending = start.pending
    var end = await awaitNextcloudGrant(
        poll: {
            await Task.detached(priority: .userInitiated) {
                Result { try app.pollNextcloudSignIn(pending: pending) }
            }.value
        },
        pause: { await wake.pause(for: interval) }
    )
    NotificationCenter.default.removeObserver(returned)
    // Granted, expired or retired, the page has nothing more to do: the sheet closes.
    browser.close()
    if case let .granted(grant) = end, Task.isCancelled {
        app.abandonNextcloudSignIn(grant: grant.value)
        end = .cancelled
    }
    logBrowserSignIn("nextcloud", flightLog(end))
    return end
}

private func flightLog(_ end: NextcloudFlightEnd) -> String {
    switch end {
    case .granted: "access granted"
    case .expired: "the sign-in was not finished in time"
    case .failed: "the sign-in ended without a grant"
    case .cancelled: "the sign-in was retired"
    }
}

/// The login page, in the same in-app browser tab the other sign-ins use, so a person already
/// signed in to their Nextcloud in Safari is not asked again. No redirect ever comes: the session
/// ends when the person closes it, or when the flight closes it.
@MainActor
final class NextcloudBrowser: NSObject, ASWebAuthenticationPresentationContextProviding {
    #if os(macOS)
    static let activated = NSApplication.didBecomeActiveNotification
    #else
    static let activated = UIApplication.didBecomeActiveNotification
    #endif

    private var session: ASWebAuthenticationSession?
    private var closing = false

    /// Opens `url`, reporting through `ended` when the person closed it or it failed. Answers
    /// whether it opened.
    func open(_ url: String, ended: @escaping @MainActor (NextcloudWakeSignal) -> Void) -> Bool {
        guard let parsed = URL(string: url) else { return false }
        let session = ASWebAuthenticationSession(
            url: parsed, callbackURLScheme: Brand.appID
        ) { @Sendable _, error in
            let closedByPerson = (error as? ASWebAuthenticationSessionError)?.code == .canceledLogin
            Task { @MainActor in
                guard !self.closing else { return }
                ended(closedByPerson ? .closed : .browserFailed)
            }
        }
        session.presentationContextProvider = self
        session.prefersEphemeralWebBrowserSession = false
        self.session = session
        logBrowserSignIn("nextcloud", "opening the browser")
        guard session.start() else {
            logBrowserSignIn("nextcloud", "the browser did not open")
            return false
        }
        return true
    }

    func close() {
        closing = true
        session?.cancel()
        session = nil
    }

    func presentationAnchor(for _: ASWebAuthenticationSession) -> ASPresentationAnchor {
        #if os(macOS)
        return NSApp.keyWindow ?? NSApp.windows.first ?? ASPresentationAnchor()
        #else
        let keyWindow = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows)
            .first { $0.isKeyWindow }
        return keyWindow ?? ASPresentationAnchor()
        #endif
    }
}
