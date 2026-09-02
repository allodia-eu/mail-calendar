// Becoming the OS's default mail app on Apple platforms: what this build can do about it, whether
// it is already true, and the one call that changes it.
//
// The *policy*, when to ask and remembering the answer, is the shared core's
// (`shouldOfferDefaultMailApp`, `recordDefaultMailAppOffer`). This file is only the platform half,
// and it is the half that differs most between two builds of the same source. See
// docs/os-integration.md.

#if os(macOS)
import AppKit
#endif
import Foundation
import MailcalBindings

enum DefaultMailApp {

    /// The scheme a mail app handles, and the one this app registers for in its Info.plist
    /// (`CFBundleURLTypes`). Registering is what makes the OS *offer* the app; it is not what
    /// makes it the default.
    static let scheme = "mailto"

    /// What this build can do about becoming the default.
    ///
    /// ⚠️ This is a property of the **build**, not of macOS. `setDefaultApplication` is blocked by
    /// the App Sandbox, so the Mac App Store build cannot ask at all, while the Developer ID build
    /// of the same source can. Apple's own guidance is that there is no replacement and no
    /// workaround, so the App Store build reports `unsupported` and never shows the offer: a
    /// prompt that can only fail is worse than no prompt.
    ///
    /// iOS/iPadOS is `unsupported` for a different reason, and only for now: appearing in
    /// Settings → Apps → Default Apps needs the `com.apple.developer.mail-client` entitlement,
    /// which Apple grants by request. Until this build carries it, offering would send the user to
    /// a list this app is not in.
    static var support: DefaultMailAppSupport {
        #if os(macOS)
        isSandboxed ? .unsupported : .setDirectly
        #else
        .unsupported
        #endif
    }

    /// Whether this app is already the handler, or `nil` where that cannot be determined.
    ///
    /// The core treats `nil` as "not the default", which is the recoverable way round: offering
    /// where we need not costs one dismissible prompt, and staying silent where we are not the
    /// default is the state the whole feature exists to change.
    static var isDefault: Bool? {
        #if os(macOS)
        guard let handler = NSWorkspace.shared.urlForApplication(toOpen: URL(string: "\(scheme):")!)
        else {
            return nil
        }
        // Resolved and standardised on both sides: two URLs naming one bundle are unequal as
        // values when one carries a trailing slash or an unresolved symlink (`/var` for
        // `/private/var`), which would read as "not the default" for the app that is.
        return handler.resolvingSymlinksInPath().standardizedFileURL
            == Bundle.main.bundleURL.resolvingSymlinksInPath().standardizedFileURL
        #else
        // iOS has no API to ask, by design: an app cannot enumerate what handles a scheme.
        return nil
        #endif
    }

    /// Asks the OS to make this app the default mail app, reporting whether the request was made.
    ///
    /// macOS shows its own consent alert, so this never changes anything on its own; `false` means
    /// the request could not even be put, which is what the sandbox does.
    ///
    /// The completion is not awaited: the alert is the user's, and the answer we care about, "did
    /// they take the offer", is recorded from the button they pressed in *our* prompt, not from
    /// this call. Treating the system alert's outcome as the answer would mean the offer came
    /// back the next launch every time someone thought about it and said no.
    @discardableResult
    static func requestDefault() -> Bool {
        #if os(macOS)
        guard support == .setDirectly else { return false }
        NSWorkspace.shared.setDefaultApplication(
            at: Bundle.main.bundleURL,
            toOpenURLsWithScheme: scheme
        ) { _ in }
        return true
        #else
        return false
        #endif
    }

    #if os(macOS)
    /// Whether this process runs under the App Sandbox.
    ///
    /// Read from the container environment rather than by inspecting the code signature: the
    /// variable is present for every sandboxed process and absent otherwise, it needs no
    /// entitlement to read, and it cannot disagree with the sandbox the process is actually in.
    private static var isSandboxed: Bool {
        ProcessInfo.processInfo.environment["APP_SANDBOX_CONTAINER_ID"] != nil
    }
    #endif
}
