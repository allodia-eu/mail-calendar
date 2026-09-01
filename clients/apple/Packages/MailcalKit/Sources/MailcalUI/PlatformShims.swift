// Small cross-platform shims that collapse the macOS/iOS divergences the shared views need, so
// the call sites stay platform-agnostic. This is the one place `#if os()` should proliferate.
import SwiftUI

#if os(macOS)
import AppKit
/// AppKit/UIKit representable + view aliases, so a single representable body serves both platforms.
public typealias PlatformViewRepresentable = NSViewRepresentable
typealias PlatformView = NSView
#else
import QuickLook
import UIKit
import UniformTypeIdentifiers
public typealias PlatformViewRepresentable = UIViewRepresentable
typealias PlatformView = UIView
#endif

extension View {
    /// A radio-group picker on macOS; its closest analogue, an inline list, on iOS/iPadOS.
    @ViewBuilder
    func radioPickerStyle() -> some View {
        #if os(macOS)
        pickerStyle(.radioGroup)
        #else
        pickerStyle(.inline)
        #endif
    }

    /// Configures a text field for the kind of value it holds: disables autocapitalization and
    /// autocorrect and picks the right software keyboard + content type on iOS (so a mail server
    /// isn't "helpfully" capitalized to `Imap.…` and an address gets the `@`/`.` email keyboard).
    /// A no-op-but-autocorrect-off on macOS, where the OS keyboard settings don't apply.
    @ViewBuilder
    func fieldConfig(_ kind: TextFieldKind) -> some View {
        #if os(iOS)
        switch kind {
        case .host:
            textInputAutocapitalization(.never)
                .autocorrectionDisabled(true)
                .keyboardType(.URL)
                .textContentType(.URL)
        case .email:
            textInputAutocapitalization(.never)
                .autocorrectionDisabled(true)
                .keyboardType(.emailAddress)
                .textContentType(.emailAddress)
        case .password:
            textInputAutocapitalization(.never)
                .autocorrectionDisabled(true)
                .textContentType(.password)
        }
        #else
        autocorrectionDisabled(true)
        #endif
    }
}

/// The kind of value a text field holds, so `fieldConfig` can pick the right keyboard/content type.
enum TextFieldKind {
    case host      // a server hostname, URL keyboard, no capitalization
    case email     // an email address, email keyboard
    case password  // a secret, password autofill, no capitalization
}

/// Puts `text` on the system clipboard, NSPasteboard on macOS, UIPasteboard on iOS. Used by
/// the Diagnostics settings' "Copy path".
enum PlatformPasteboard {
    static func copy(_ text: String) {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        #else
        UIPasteboard.general.string = text
        #endif
    }
}

/// One link in a chain of modal presentations. Abstracted from `UIViewController` for one reason:
/// the walk below is the fix for a shipped bug, and `swift test` runs on **macOS**, an
/// `#if os(iOS)` test of it would compile to nothing and report a pass over untested code.
///
/// Main-actor bound because a `UIViewController` is, and a nonisolated protocol cannot take that
/// conformance without opening a hole the walk would sit in.
@MainActor
protocol PresentationChainNode: AnyObject {
    /// What this node is currently presenting, if anything.
    var nextPresented: (any PresentationChainNode)? { get }
    /// Whether this node is mid-dismissal, and so must not be presented from.
    var isDismissingNow: Bool { get }
}

/// Walks a presentation chain to the controller a new modal must actually be presented *from*.
///
/// UIKit refuses, **silently**, with no error and no callback, a second `present` on a controller
/// that is already presenting. Every modal this app raises is asked for from inside another one:
/// Attach files from the composer (a `fullScreenCover`), the file picker again from the
/// add-account sheet, the log share from the Diagnostics settings. Presenting from the *root*
/// therefore does nothing at all, which is exactly how it gets reported, "tapped it, no response".
@MainActor
func topOfPresentationChain(from root: any PresentationChainNode) -> any PresentationChainNode {
    var host = root
    while let next = host.nextPresented, !next.isDismissingNow {
        host = next
    }
    return host
}

#if os(iOS)
extension UIViewController: PresentationChainNode {
    var nextPresented: (any PresentationChainNode)? { presentedViewController }
    var isDismissingNow: Bool { isBeingDismissed }
}

/// The iOS analogue of macOS's "save panel": a share sheet, which offers "Save to Files" and every
/// other destination. Opening is `PlatformQuickLook`, a share sheet is where a file goes next, not
/// a way to look at it.
enum PlatformShare {
    @MainActor static func present(_ url: URL) {
        guard let host = topPresentedViewController() else { return }
        let sheet = UIActivityViewController(activityItems: [url], applicationActivities: nil)
        if let pop = sheet.popoverPresentationController {
            pop.sourceView = host.view
            pop.sourceRect = CGRect(x: host.view.bounds.midX, y: host.view.bounds.midY, width: 0, height: 0)
        }
        host.present(sheet, animated: true)
    }
}

/// The iOS analogue of `NSWorkspace.open`: Quick Look, the system viewer Files and Mail preview
/// with. The content is rendered by the OS's own out-of-process preview extensions, never by this
/// app, the same reason macOS hands the file to the default handler (see
/// docs/rendering-security.md). Retains itself for the lifetime of the preview.
@MainActor
final class PlatformQuickLook: NSObject, QLPreviewControllerDataSource, QLPreviewControllerDelegate {
    private static var active: PlatformQuickLook?
    private let url: URL
    private init(url: URL) { self.url = url }

    /// Presents `url` in Quick Look. Returns `false` when the OS declines the type outright (an
    /// installer, an unknown binary) or there is nothing to present from, so the caller can fall
    /// back. A type it accepts but cannot draw, an archive, gets its own "no preview" card, which
    /// carries a Share button, so it never reaches the fallback and needs none.
    static func present(_ url: URL) -> Bool {
        guard QLPreviewController.canPreview(url as NSURL), let host = topPresentedViewController()
        else { return false }
        let source = PlatformQuickLook(url: url)
        active = source
        let preview = QLPreviewController()
        preview.dataSource = source
        preview.delegate = source
        host.present(preview, animated: true)
        return true
    }

    func numberOfPreviewItems(in controller: QLPreviewController) -> Int { 1 }

    func previewController(
        _ controller: QLPreviewController,
        previewItemAt index: Int
    ) -> any QLPreviewItem {
        url as NSURL
    }

    /// `nonisolated` because `QLPreviewControllerDelegate` is, unlike the data source beside it,
    /// not declared on the main actor.
    ///
    /// A hop rather than `MainActor.assumeIsolated`: all this does is drop the retain that kept the
    /// preview alive, so it loses nothing by happening a moment later, and an assumption that the
    /// header does not actually promise would be a crash if QuickLook ever called it elsewhere.
    ///
    /// Identity-checked, because the hop is not synchronous: a preview opened between the dismissal
    /// and this landing owns `active` by then, and `QLPreviewController` holds its data source
    /// weakly, so clearing it blindly would deallocate the preview the user is looking at.
    nonisolated func previewControllerDidDismiss(_ controller: QLPreviewController) {
        Task { @MainActor in
            if Self.active === self { Self.active = nil }
        }
    }
}

/// The iOS analogue of `NSOpenPanel`: a document picker that copies the chosen files in and
/// returns their URLs. Retains itself for the lifetime of the picker.
@MainActor
final class PlatformFilePicker: NSObject, UIDocumentPickerDelegate {
    private static var active: PlatformFilePicker?
    private let onPick: ([URL]) -> Void
    private init(onPick: @escaping ([URL]) -> Void) { self.onPick = onPick }

    static func present(onPick: @escaping ([URL]) -> Void) {
        guard let host = topPresentedViewController() else { return }
        let picker = PlatformFilePicker(onPick: onPick)
        active = picker
        let vc = UIDocumentPickerViewController(forOpeningContentTypes: [.item], asCopy: true)
        vc.allowsMultipleSelection = true
        vc.delegate = picker
        host.present(vc, animated: true)
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        onPick(urls)
        Self.active = nil
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        Self.active = nil
    }
}

/// The view controller a modal must be presented *from*, the top of the key window's presentation
/// chain, never the root. See `topOfPresentationChain` for why the difference is load-bearing.
@MainActor private func topPresentedViewController() -> UIViewController? {
    guard let root = UIApplication.shared.connectedScenes
        .compactMap({ $0 as? UIWindowScene })
        .flatMap(\.windows)
        .first(where: { $0.isKeyWindow })?
        .rootViewController
    else { return nil }
    // Every node in a UIKit presentation chain is a UIViewController, so this cast always holds.
    return topOfPresentationChain(from: root) as? UIViewController
}
#endif
