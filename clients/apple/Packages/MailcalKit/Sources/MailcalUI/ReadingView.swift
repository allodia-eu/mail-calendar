// The message reading view: renders the open message's body that the Rust core fetched and
// (for HTML) sanitised. The HTML document, its strict CSP, base styling, and remote-image
// gating, is built in shared Rust (`renderMessageHtml`) so every client behaves
// identically; this view only supplies the unavoidably-native bits: a WKWebView with
// JavaScript off and in-view navigation blocked (clicked links open in the default
// browser instead), plus the plain-text fallback. Remote images are
// blocked by default behind a "load remote images" confirmation. Split into its own file to
// keep Mailcal.swift under the 500-line limit.
//
// The security gates here are a CROSS-PLATFORM CONTRACT, see docs/rendering-security.md. Any
// gate added/raised on one platform must be applied to all of them (and recorded there).

#if os(macOS)
import AppKit
#else
import UIKit
#endif
import MailcalBindings
import SwiftUI
import WebKit

/// The header context for an opened message (the row the user tapped). The body itself is
/// pulled from the model's `reading` snapshot, matched by `key`.
struct OpenedMessage: Identifiable, Hashable {
    /// The id of the account that owns the message, passed back into reading/reply/
    /// forward intents so they route to the owning account (two accounts can share a key).
    let account: String
    let key: String
    let subject: String
    let from: String
    /// The sender's avatar as the list row already had it, so the header draws a face
    /// immediately rather than flashing empty until the body snapshot arrives.
    let avatar: Avatar
    let date: String
    var id: String { key }
}
/// How much of the screen the attachment bar may take before it starts scrolling inside itself.
///
/// Roughly four rows. The bar sits above the message in a column that does not scroll, so this is
/// the line between "this message has attachments" and "this message is now unreadable".
///
/// Not `private`: ReadingView.Attachments.swift's `attachmentRows` reads it too (Swift's
/// `private` on a top-level declaration is file-scoped).
let attachmentBarCap: CGFloat = 176

/// The reading pane: a header plus the fetched body, or a spinner until the body for this
/// message arrives (the fetch is async, a network round-trip on the first open). It lives
/// inline as the third pane beside the message list (sidebar | list | reading); a fresh
/// instance is created per opened message (keyed by the selected row), so its per-message
/// state resets on selection.
struct ReadingView: View {
    var model: MailboxModel
    let message: OpenedMessage
    /// Header-toolbar actions, owned by the host (they open the composer / act on the message
    /// and clear the pane) so the reading view stays a pure renderer.
    let onReply: () -> Void
    let onReplyAll: () -> Void
    let onForward: () -> Void
    let onArchive: () -> Void
    let onDelete: () -> Void

    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var hSizeClass
    #endif
    /// A compact (iPhone) width never fits the labelled action buttons, so they render icon-only
    /// with larger tap targets. macOS/iPad decide per-row instead (see `actionToolbar`), keeping
    /// the labels whenever they fit at the pane's current width.
    private var compactActions: Bool {
        #if os(iOS)
        hSizeClass == .compact
        #else
        false
        #endif
    }

    /// Whether the user chose to load this message's remote images (reset per message, a fresh
    /// view is created for each opened message).
    @State private var loadRemoteImages = false
    /// A transient attachment save/open failure message shown in the attachment bar; nil when
    /// the last action succeeded or none has run.
    ///
    /// Not `private`: ReadingView.Attachments.swift reads and sets it too.
    @State var attachmentError: String?
    /// The attachment rows' natural height, so the bar can hug them up to `attachmentBarCap`. Not
    /// `private`, see `attachmentError`.
    @State var attachmentsHeight: CGFloat = 0
    /// Attachment ids whose Open / Save is in flight, so the tapped button shows a spinner (and
    /// ignores re-taps) while the OS launches the file or the part is decoded + written, the
    /// brief delay reads as "working" rather than a dead click. Keyed per action so Open and
    /// Save spin independently. Not `private`, see `attachmentError`.
    @State var openingIDs: Set<UInt32> = []
    @State var savingIDs: Set<UInt32> = []

    /// The body snapshot for this message, once it has arrived (ignore a stale one for a
    /// previously-opened message).
    ///
    /// Not `private`: ReadingView.Attachments.swift's `attachmentBar` reads it too.
    var bodySnapshot: ReadingSnapshot? {
        guard let reading = model.reading, reading.key == message.key else { return nil }
        return reading
    }

    /// The reading header's sender: the full `Name <email>` from the body snapshot once it
    /// arrives, falling back to the carried list-row name (name-only) until then, so the line
    /// never flashes empty on open.
    private var senderLine: String {
        if let from = bodySnapshot?.from, !from.isEmpty { return from }
        return message.from
    }

    /// The sender's avatar: the body snapshot's once it arrives, else the one the list row
    /// already carried, so the header does not flash a different face on open. Both name the
    /// same person, so the letters and colour are identical; only the photo can differ, and
    /// only until the snapshot lands. A `pending` snapshot has resolved nothing and is not the
    /// answer, see [`readingHeaderAvatar`].
    private var senderAvatar: Avatar {
        readingHeaderAvatar(snapshot: bodySnapshot, row: message.avatar)
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .top) {
                AvatarView(avatar: senderAvatar, diameter: 40)
                VStack(alignment: .leading, spacing: 2) {
                    Text(message.subject.isEmpty ? L10n.mail_no_subject() : message.subject)
                        .font(.headline).lineLimit(2)
                    Text(senderLine).font(.subheadline).foregroundStyle(.secondary)
                }
                Spacer()
                Text(message.date).font(.caption).foregroundStyle(.secondary)
            }
            .padding(.horizontal, 12)
            .padding(.top, 12)
            recipientsHeader
            actionToolbar
            attachmentBar
            // Empty unless the core's RSVP gate called this message an invitation.
            InvitationBanner(
                snapshot: bodySnapshot,
                zone: model.activeZone,
                use24Hour: model.use24Hour,
                account: message.account,
                messageKey: message.key,
                writeStatus: model.calendarWriteStatus,
                respond: { [model] response, comment, notify, replySubject in
                    model.respondToInvitation(
                        message.account,
                        message.key,
                        response,
                        comment: comment,
                        notifyOrganizer: notify,
                        replySubject: replySubject
                    )
                }
            )
            Divider()
            content
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    /// The recipient headers (To / Cc / Bcc), shown once this message's snapshot arrives.
    /// Each row appears only when non-empty; Bcc is present only on the user's own Sent/Drafts
    /// copies (whose stored message carries a Bcc header), so they can see whom they Bcc'd.
    @ViewBuilder
    private var recipientsHeader: some View {
        if let body = bodySnapshot,
            !(body.to.isEmpty && body.cc.isEmpty && body.bcc.isEmpty) {
            VStack(alignment: .leading, spacing: 1) {
                recipientRow(L10n.compose_to(), body.to)
                recipientRow(L10n.compose_cc(), body.cc)
                recipientRow(L10n.compose_bcc(), body.bcc)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 12)
            .padding(.top, 4)
        }
    }

    @ViewBuilder
    private func recipientRow(_ label: String, _ value: String) -> some View {
        if !value.isEmpty {
            HStack(alignment: .top, spacing: 4) {
                Text("\(label):").font(.caption).foregroundStyle(.secondary)
                Text(value)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                    .lineLimit(2)
            }
        }
    }

    /// The message action bar: reply/forward on the left, archive/delete on the right:
    /// mirroring the list's context menu, surfaced as buttons like a desktop mail client.
    ///
    /// Whether the labels fit is a question of *width*, not platform: a narrow reading pane, or
    /// simply a longer language (Dutch's "Allen beantwoorden" against English's "Reply all"), can
    /// outgrow the row and leave every button clipped to "Beant…". So the labelled row is offered
    /// first and the icon-only row is the fallback whenever the labels can't be drawn in full.
    private var actionToolbar: some View {
        Group {
            if compactActions {
                actionRow(iconsOnly: true)
            } else {
                ViewThatFits(in: .horizontal) {
                    actionRow(iconsOnly: false)
                    actionRow(iconsOnly: true)
                }
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    private func actionRow(iconsOnly: Bool) -> some View {
        HStack(spacing: 8) {
            toolbarButton(L10n.action_reply(), "arrowshape.turn.up.left", iconsOnly, action: onReply)
            toolbarButton(L10n.action_reply_all(), "arrowshape.turn.up.left.2", iconsOnly, action: onReplyAll)
            toolbarButton(L10n.action_forward(), "arrowshape.turn.up.right", iconsOnly, action: onForward)
            // A flexible Spacer would let any row "fit", so ViewThatFits could never reject the
            // labelled one; a minimum length gives the row an honest ideal width to measure.
            Spacer(minLength: 12)
            toolbarButton(L10n.action_archive(), "archivebox", iconsOnly, action: onArchive)
            toolbarButton(L10n.action_delete(), "trash", iconsOnly, role: .destructive, action: onDelete)
        }
    }

    private func toolbarButton(
        _ title: String,
        _ icon: String,
        _ iconsOnly: Bool,
        role: ButtonRole? = nil,
        action: @escaping () -> Void
    ) -> some View {
        Button(role: role, action: action) {
            if iconsOnly {
                Image(systemName: icon).frame(minWidth: 24, minHeight: 24)
            } else {
                // One line at its natural width, so the row's ideal width is what ViewThatFits
                // measures, never a silently truncated or hyphenated label.
                Label(title, systemImage: icon)
                    .lineLimit(1)
                    .fixedSize(horizontal: true, vertical: false)
            }
        }
        .buttonStyle(.bordered)
        .controlSize(compactActions ? .large : .small)
        .accessibilityLabel(title)
    }

    @ViewBuilder
    private var content: some View {
        if let body = bodySnapshot {
            if body.pending {
                // The core publishes this only once an open has run long enough to be worth
                // announcing, so the indicator appears for a wait and never for a fast open.
                // It carries no body, so this has to come before the branches that read one:
                // an empty `pending` snapshot is not a message without content.
                VStack(spacing: 10) {
                    ProgressView()
                    Text(L10n.reading_loading()).font(.caption).foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if body.loadError {
                VStack(spacing: 10) {
                    Image(systemName: "exclamationmark.triangle").foregroundStyle(.secondary)
                    Text(L10n.reading_load_error()).foregroundStyle(.secondary)
                    Button(L10n.action_retry()) { model.openMessage(message.account, message.key) }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let html = body.html, !html.isEmpty {
                if body.hasRemoteImages && !loadRemoteImages {
                    RemoteImagesBanner { loadRemoteImages = true }
                }
                SanitizedHTMLView(fragment: html, loadRemoteImages: loadRemoteImages)
            } else if let plain = body.plain, !plain.isEmpty {
                ScrollView {
                    Text(plain)
                        .font(.body)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(12)
                }
            } else {
                placeholder(L10n.reading_no_content())
            }
        } else {
            // Opened, and nothing to say yet. Not a spinner: the body usually arrives within a
            // few milliseconds, and one drawn on every open flickers rather than reassures. The
            // header above is already filled from the row that was tapped, so the pane reads as
            // the message opening rather than as empty.
            Color.clear.frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func placeholder(_ text: String) -> some View {
        Text(text)
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// The reading pane's empty state: shown in the third pane when no message is selected, so the
/// column reads as "pick a message" rather than sitting blank (the desktop master-detail idiom).
struct ReadingPanePlaceholder: View {
    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: "envelope.open")
                .font(.system(size: 32))
                .foregroundStyle(.tertiary)
            Text(L10n.reading_empty())
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// The bar shown above a message that has remote images, which are blocked by default to
/// avoid tracking. Tapping "Load images" opts in for this message.
private struct RemoteImagesBanner: View {
    let onLoad: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "photo").foregroundStyle(.secondary)
            Text(L10n.reading_remote_blocked())
                .font(.caption)
            Spacer()
            Button(L10n.action_load_images(), action: onLoad)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.quaternary)
    }
}

/// Renders the core's sanitised HTML in a hardened WKWebView. The full document (strict CSP,
/// base styling, remote-image gating) is produced by shared Rust (`renderMessageHtml`); this
/// adds the native defenses: JavaScript disabled and in-view navigation blocked (a clicked
/// link opens in the default browser rather than loading inside the inert document).
struct SanitizedHTMLView: PlatformViewRepresentable {
    let fragment: String
    let loadRemoteImages: Bool

    func makeCoordinator() -> Coordinator { Coordinator() }

    #if os(macOS)
    func makeNSView(context: Context) -> WKWebView { makeWebView(context) }
    func updateNSView(_ webView: WKWebView, context: Context) { updateWebView(webView, context) }
    #else
    func makeUIView(context: Context) -> WKWebView { makeWebView(context) }
    func updateUIView(_ webView: WKWebView, context: Context) { updateWebView(webView, context) }
    #endif

    private func makeWebView(_ context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.defaultWebpagePreferences.allowsContentJavaScript = false
        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator
        return webView
    }

    private func updateWebView(_ webView: WKWebView, _ context: Context) {
        // Skip entirely when the inputs are unchanged, so unrelated SwiftUI updates don't
        // re-run the (FFI) document build or reload the page, only the fragment or the
        // load-images choice changing matters.
        let coordinator = context.coordinator
        guard coordinator.lastFragment != fragment
            || coordinator.lastLoadRemoteImages != loadRemoteImages
        else { return }
        coordinator.lastFragment = fragment
        coordinator.lastLoadRemoteImages = loadRemoteImages
        // baseURL nil → no origin to resolve remote/relative resources against; combined
        // with the document's CSP this guarantees no network access beyond opted-in images.
        let document = renderMessageHtml(html: fragment, loadRemoteImages: loadRemoteImages)
        webView.loadHTMLString(document, baseURL: nil)
    }

    final class Coordinator: NSObject, WKNavigationDelegate {
        var lastFragment: String?
        var lastLoadRemoteImages: Bool?

        func webView(
            _ webView: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
        ) {
            // Allow only the initial in-document load (loadHTMLString → `.other`, an
            // about:/empty URL); the body itself is inert, we never navigate it in place.
            let url = navigationAction.request.url
            if navigationAction.navigationType == .other,
                url == nil || url?.scheme == "about" {
                decisionHandler(.allow)
                return
            }
            // A link the user clicked opens in their default browser/handler instead; the
            // in-view navigation is still cancelled, so the document stays inert. The
            // allow-or-ignore decision is the shared-Rust launch policy (`shouldOpenExternalLink`)
            // so every client is identical, see docs/rendering-security.md.
            if navigationAction.navigationType == .linkActivated,
                let url, shouldOpenExternalLink(url: url.absoluteString) {
                #if os(macOS)
                NSWorkspace.shared.open(url)
                #else
                UIApplication.shared.open(url)
                #endif
            }
            decisionHandler(.cancel)
        }
    }
}
