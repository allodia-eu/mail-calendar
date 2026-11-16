// The Share Extension: what the OS runs when someone picks this app out of a share sheet
// (docs/os-integration.md).
//
// It draws nothing while it works. A share extension normally presents a small compose sheet of
// its own, and this one cannot: a composer needs the accounts, the signatures and the send path,
// and those belong to the one core with the one open store. A second process holding a second core
// would be a second writer on the user's mail (docs/reading-window.md). So the extension does the
// one thing only it can do, copy the shared bytes somewhere the app may read them, and then asks
// for the app. Only where the system will not bring the app up does it draw, to say where the
// share is waiting.
//
// Nothing here is ever sent, and nothing here decides what a share means: the app hands the drop to
// `prefill_from_share`, which owns the names, the media types, the cap and the refusals.

import Foundation
import MailcalShareBox

#if os(macOS)
import AppKit
typealias ShareHostController = NSViewController
#else
import SwiftUI
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

    override func viewDidLoad() {
        super.viewDidLoad()
        handOver()
    }
    #else
    private var handedOver = false

    /// Once on screen rather than on load: the app is opened through the responder chain, which
    /// reaches the application only once the view is in a window.
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        guard !handedOver else { return }
        handedOver = true
        handOver()
    }
    #endif

    /// Stages the share, leaves a note for the app, and asks for it.
    ///
    /// The request is completed always, however little was taken: the sharing app's own window
    /// stays blocked until it is, so a path that forgets leaves the *other* app looking hung.
    private func handOver() {
        guard let context = extensionContext else { return }
        let items = (context.inputItems as? [NSExtensionItem]) ?? []
        Task {
            var deposited = false
            if let box = ShareBox.shared(appID: Self.appID, dataDirName: Self.dataDirName),
                let drop = await ShareIntake.read(items: items, into: box) {
                deposited = (try? box.deposit(drop)) != nil
            }
            guard deposited, !(await openApp()) else {
                context.completeRequest(returningItems: [], completionHandler: nil)
                return
            }
            #if os(macOS)
            context.completeRequest(returningItems: [], completionHandler: nil)
            #else
            explain { context.completeRequest(returningItems: [], completionHandler: nil) }
            #endif
        }
    }

    /// Asks the system to bring the app up, answering whether it did.
    ///
    /// Best effort: the drop is already in the box, and the app drains it every time it is
    /// activated, so an unanswered doorbell costs the user a launch of their own rather than the
    /// files they shared.
    ///
    /// ⚠️ **Neither platform goes through the extension context.** `NSExtensionContext.open` asks
    /// the *host* to open the URL, and neither host does: Finder left the app unlaunched (MEASURED
    /// 2026-09-20), and iOS honours the call for a Today widget alone, answering `false` to a share
    /// extension. macOS asks LaunchServices itself, which a sandboxed process may; iOS asks the
    /// extension's own application object (`openThroughApplication`).
    private func openApp() async -> Bool {
        guard let doorbell = ShareHandoff.doorbell(appID: Self.appID) else { return false }
        #if os(macOS)
        return (try? await NSWorkspace.shared.open(
            doorbell, configuration: NSWorkspace.OpenConfiguration())) != nil
        #else
        return await openThroughApplication(doorbell)
        #endif
    }

    #if os(iOS)
    /// Opens `url` through the `UIApplication` this extension runs in.
    ///
    /// Reached through the responder chain because `UIApplication.shared` is unavailable to an
    /// extension, and called by selector because its `open` is marked unavailable here too. Apple
    /// does not document it for an extension, so a refusal is expected one day and is answered by
    /// `explain`, never by a share that silently goes nowhere.
    private func openThroughApplication(_ url: URL) async -> Bool {
        var responder: UIResponder? = self
        while let current = responder, !(current is UIApplication) { responder = current.next }
        let selector = NSSelectorFromString("openURL:options:completionHandler:")
        guard let application = responder as? UIApplication, application.responds(to: selector)
        else { return false }
        typealias Open = @convention(c) (
            AnyObject, Selector, URL, NSDictionary, (@convention(block) @Sendable (Bool) -> Void)?
        ) -> Void
        let open = unsafeBitCast(application.method(for: selector), to: Open.self)
        return await withCheckedContinuation { continuation in
            open(application, selector, url, NSDictionary()) { continuation.resume(returning: $0) }
        }
    }

    /// Says the share is waiting in the app, for a system that would not open it, and finishes
    /// when the person is done reading.
    private func explain(then finish: @escaping @MainActor () -> Void) {
        let note = UIHostingController(rootView: ShareReadyNote(appName: Self.appName, done: finish))
        addChild(note)
        note.view.frame = view.bounds
        note.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(note.view)
        note.didMove(toParent: self)
    }
    #endif

    /// What the operating system knows the app by (docs/branding.md).
    ///
    /// Read from this bundle's own Info.plist, where XcodeGen injected it, rather than taken from
    /// `MailcalBindings`' generated `Brand`: linking that would pull the Rust core into an
    /// extension whose whole job is to copy a file, and iOS holds an extension to a far smaller
    /// memory budget than an app.
    private static let appID: String =
        Bundle.main.object(forInfoDictionaryKey: "MailcalAppID") as? String ?? ""

    /// The app's data directory name, which the drop box is named after; injected the same way.
    private static let dataDirName: String =
        Bundle.main.object(forInfoDictionaryKey: "MailcalDataDirName") as? String ?? ""

    /// The product's name, which the extension's display name is (project.yml).
    private static let appName: String =
        Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String ?? ""
}

#if os(iOS)
/// The note a share leaves when the app could not be opened for it.
private struct ShareReadyNote: View {
    let appName: String
    let done: @MainActor () -> Void

    var body: some View {
        NavigationStack {
            VStack(spacing: 12) {
                Image(systemName: "envelope.badge")
                    .font(.largeTitle)
                    .foregroundStyle(.tint)
                    .accessibilityHidden(true)
                Text(L10n.share_ready_title(app: appName))
                    .font(.title3.weight(.semibold))
                    .multilineTextAlignment(.center)
                Text(L10n.share_ready_body(app: appName))
                    .multilineTextAlignment(.center)
                    .foregroundStyle(.secondary)
            }
            .padding(24)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(L10n.action_done()) { done() }
                }
            }
        }
    }
}
#endif
