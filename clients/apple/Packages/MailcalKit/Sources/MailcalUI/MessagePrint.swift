// Printing the open message (docs/reading-actions.md, "Printing a message"). The page is built in
// shared Rust; what is native is the web view it is laid out in, which carries the reading view's
// gates, and the platform's print dialog.

#if os(macOS)
import AppKit
#else
import UIKit
#endif
import MailcalBindings
import WebKit

/// The document to print for `message`, or `nil` while there is no body to print: the open is still
/// running, or the body could not be fetched.
///
/// The lines are the ones the reading header draws, under the same labels, so a printout says what
/// the screen said. `loadRemoteImages` is the reader's choice for this message.
func messagePrintDocument(
    _ message: OpenedMessage,
    _ snapshot: ReadingSnapshot?,
    loadRemoteImages: Bool
) -> String? {
    guard let snapshot, !snapshot.pending, !snapshot.loadError else { return nil }
    return renderMessagePrintHtml(
        subject: message.subject.isEmpty ? L10n.mail_no_subject() : message.subject,
        lines: [
            PrintHeaderLine(label: L10n.compose_from(), value: snapshot.from.isEmpty ? message.from : snapshot.from),
            PrintHeaderLine(label: L10n.compose_to(), value: snapshot.to),
            PrintHeaderLine(label: L10n.compose_cc(), value: snapshot.cc),
            PrintHeaderLine(label: L10n.compose_bcc(), value: snapshot.bcc),
            PrintHeaderLine(label: L10n.quote_sent(), value: message.date),
        ],
        html: snapshot.html,
        plain: snapshot.plain,
        loadRemoteImages: loadRemoteImages
    )
}

/// Lays a print document out in a web view nobody sees, then hands it to the print dialog.
///
/// The web view is the reading view's own (`makeReadingWebView`), so scripting is off, and this
/// delegate refuses every navigation but the document's own load.
@MainActor
final class MessagePrinter: NSObject, WKNavigationDelegate {
    /// The print in flight. The dialog borrows the web view rather than owning it, so something has
    /// to hold it until the dialog is done.
    private static var current: MessagePrinter?

    private let webView = makeReadingWebView()
    private let jobName: String

    private init(jobName: String) {
        self.jobName = jobName
    }

    static func print(_ document: String, jobName: String) {
        let printer = MessagePrinter(jobName: jobName)
        current = printer
        printer.webView.navigationDelegate = printer
        // Never on screen, so nothing else gives it a size to lay the page out at.
        printer.webView.frame = CGRect(x: 0, y: 0, width: 640, height: 800)
        loadReadingDocument(printer.webView, document)
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        present()
    }

    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void
    ) {
        let url = navigationAction.request.url
        let initialLoad = navigationAction.navigationType == .other && (url == nil || url?.scheme == "about")
        decisionHandler(initialLoad ? .allow : .cancel)
    }

    #if os(macOS)
    private func present() {
        let operation = webView.printOperation(with: NSPrintInfo.shared)
        operation.jobTitle = jobName
        operation.view?.frame = webView.bounds
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow else {
            Self.current = nil
            return
        }
        // A sheet on the window the reader pressed Print in, the pane's or a reading window's.
        operation.runModal(
            for: window,
            delegate: self,
            didRun: #selector(printOperationDidRun(_:success:contextInfo:)),
            contextInfo: nil
        )
    }

    @objc private func printOperationDidRun(
        _ operation: NSPrintOperation,
        success: Bool,
        contextInfo: UnsafeMutableRawPointer?
    ) {
        Self.current = nil
    }
    #else
    private func present() {
        let info = UIPrintInfo(dictionary: nil)
        info.jobName = jobName
        info.outputType = .general
        let controller = UIPrintInteractionController.shared
        controller.printInfo = info
        controller.printFormatter = webView.viewPrintFormatter()
        controller.present(animated: true) { _, _, _ in
            Self.current = nil
        }
    }
    #endif
}
