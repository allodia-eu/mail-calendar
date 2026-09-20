// The Share Extension: what the OS runs when someone picks this app out of a share sheet
// (docs/os-integration.md).
//
// It draws nothing. A share extension normally presents a small compose sheet of its own, and this
// one cannot: a composer needs the accounts, the signatures and the send path, and those belong to
// the one core with the one open store. A second process holding a second core would be a second
// writer on the user's mail (docs/reading-window.md). So the extension does the one thing only it
// can do, copy the shared bytes somewhere the app may read them, and then asks for the app.
//
// Nothing here is ever sent, and nothing here decides what a share means: the app hands the drop to
// `prefill_from_share`, which owns the names, the media types, the cap and the refusals.

import Foundation
import MailcalShareBox

#if os(macOS)
import AppKit
typealias ShareHostController = NSViewController
#else
import UIKit
typealias ShareHostController = UIViewController
#endif

final class ShareViewController: ShareHostController {

    #if os(macOS)
    // A share extension on macOS is an `NSViewController`, and one with neither a nib nor a
    // `loadView` traps before it runs. An empty view is the honest shape here: the popover appears
    // and goes in the same breath, because everything this extension does is invisible.
    override func loadView() {
        view = NSView(frame: .zero)
    }
    #endif

    override func viewDidLoad() {
        super.viewDidLoad()
        handOver()
    }

    /// Stages the share, leaves a note for the app, and rings its doorbell.
    ///
    /// The request is completed last and always, however little was taken: the sharing app's own
    /// window stays blocked until it is, so a path that forgets leaves the *other* app looking
    /// hung.
    private func handOver() {
        guard let context = extensionContext else { return }
        let items = (context.inputItems as? [NSExtensionItem]) ?? []
        Task {
            if let box = ShareBox.shared(appID: Self.appID),
                let drop = await ShareIntake.read(items: items, into: box) {
                try? box.deposit(drop)
            }
            await openApp(context)
            context.completeRequest(returningItems: [], completionHandler: nil)
        }
    }

    /// Asks the system to bring the app up.
    ///
    /// Best effort, deliberately: the drop is already in the box, and the app drains it every time
    /// it is activated, so an unanswered doorbell costs the user a launch of their own rather than
    /// the files they shared.
    ///
    /// ⚠️ **macOS goes through `NSWorkspace`, not through the extension context.**
    /// `NSExtensionContext.open` asks the *host application* to open the URL on the extension's
    /// behalf, and Finder does not: MEASURED 2026-09-20, sharing from Finder staged both files and
    /// left the app unlaunched, while the same URL passed to `open(1)` brought it straight up. A
    /// sandboxed process may still ask LaunchServices for a URL itself, which is what this is.
    /// iOS keeps the context call, where the host is UIKit's own share sheet and it works.
    private func openApp(_ context: NSExtensionContext) async {
        guard let doorbell = ShareHandoff.doorbell(appID: Self.appID) else { return }
        #if os(macOS)
        _ = try? await NSWorkspace.shared.open(
            doorbell, configuration: NSWorkspace.OpenConfiguration())
        #else
        await withCheckedContinuation { continuation in
            context.open(doorbell) { _ in continuation.resume() }
        }
        #endif
    }

    /// What the operating system knows the app by (docs/branding.md).
    ///
    /// Read from this bundle's own Info.plist, where XcodeGen injected it, rather than taken from
    /// `MailcalBindings`' generated `Brand`: linking that would pull the Rust core into an
    /// extension whose whole job is to copy a file, and iOS holds an extension to a far smaller
    /// memory budget than an app.
    private static let appID: String =
        Bundle.main.object(forInfoDictionaryKey: "MailcalAppID") as? String ?? ""
}
