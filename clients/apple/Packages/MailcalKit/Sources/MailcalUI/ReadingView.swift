// The message reading view: the header, the action row and the body the Rust core fetched and
// (for HTML) sanitised. The HTML document, its strict CSP, base styling, and remote-image
// gating, is built in shared Rust (`renderMessageHtml`) so every client behaves identically;
// the web view that hosts it is in ReadingWebView.swift. Remote images are blocked by
// default behind a "load remote images" confirmation. Split into its own file to keep
// Mailcal.swift under the 500-line limit.
//
// The security gates here are a CROSS-PLATFORM CONTRACT, see docs/rendering-security.md. Any
// gate added/raised on one platform must be applied to all of them (and recorded there).

import MailcalBindings
import SwiftUI

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

/// How far from the pane's edges the reading view draws, one value so the header, the recipients,
/// the action row, the attachment bar, the invitation card and a plain-text body all start on the
/// same vertical line. The HTML body's own inset is in the shared renderer's stylesheet, where
/// every client picks it up (`crates/mailcal-app/src/html`).
///
/// Not `private`: ReadingView.Attachments.swift and InvitationCardView.swift inset to it too
/// (Swift's `private` on a top-level declaration is file-scoped).
let readingInset: CGFloat = 16

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
    /// at a touch size. macOS/iPad measure the labels instead (see `actionToolbar`), keeping them
    /// whenever they fit at the pane's current width.
    private var compactActions: Bool {
        #if os(iOS)
        hSizeClass == .compact
        #else
        false
        #endif
    }

    /// The square an action button's glyph is drawn in, which is what decides the button's size
    /// once the style's own padding is added: 32 pt puts a touch row's buttons at about 44, the
    /// smallest a finger is served by, and the desktops keep the 24 a pointer needs.
    ///
    /// It is the **glyph's** box rather than a floor on the button, deliberately. A `frame` whose
    /// bounds are all `nil` is an unbounded `_FlexFrameLayout`, and `ViewThatFits` measuring one
    /// hangs the app before its first window (ReadingView.Overflow.swift has the same trap from
    /// the other side), so nothing here may wrap a button in a frame that is a no-op.
    ///
    /// Not `private`: ReadingView.Overflow.swift draws the overflow glyph in the same box, so it
    /// stays the same control as the buttons beside it (`docs/reading-actions.md`).
    var iconBox: CGFloat { compactActions ? 32 : 24 }

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
    /// Whether the overflow's popover is open. Not `private`, see `attachmentError`.
    @State var overflowOpen = false
    /// Whether an export is in flight, so the overflow button spins and the item ignores
    /// re-taps while the source is fetched and written. Not `private`, see `attachmentError`.
    @State var exporting = false
    /// A transient export failure, shown under the action row rather than in the attachment bar:
    /// most messages carry no attachments, and that bar is not drawn for them, so an error put
    /// there would be invisible on exactly the messages people export. Not `private`, see
    /// `attachmentError`.
    @State var exportError: String?

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
            .padding(.horizontal, readingInset)
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
            .padding(.horizontal, readingInset)
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
    ///
    /// **The control size is measured too, and on a phone it has to be.** Six icon buttons at
    /// `.large` are wider than any iPhone, and a row wider than the screen is not clipped on its
    /// own: it sets the width of the column it sits in, so the header above and the body below
    /// hang off both edges of the display. The size is set here rather than per button so
    /// `ViewThatFits` can offer the next one down, which is the same row drawn to fit, and the
    /// glyphs keep the box `iconBox` gives them, so a smaller control is a smaller *button* and
    /// not a smaller target. Icons scale with the text size, so the third candidate is what a
    /// large accessibility size lands on.
    private var actionToolbar: some View {
        VStack(alignment: .leading, spacing: 4) {
            Group {
                if compactActions {
                    ViewThatFits(in: .horizontal) {
                        actionRow(iconsOnly: true).controlSize(.large)
                        actionRow(iconsOnly: true).controlSize(.regular)
                        actionRow(iconsOnly: true).controlSize(.small)
                    }
                } else {
                    ViewThatFits(in: .horizontal) {
                        actionRow(iconsOnly: false)
                        actionRow(iconsOnly: true)
                    }
                    .controlSize(.small)
                }
            }
            if let exportError {
                Text(exportError)
                    .font(.caption)
                    .foregroundStyle(.red)
            }
        }
        .padding(.horizontal, readingInset)
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
            // Last of all, after both mailbox actions (docs/reading-actions.md).
            overflowMenu(iconsOnly: iconsOnly)
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
                Image(systemName: icon).frame(minWidth: iconBox, minHeight: iconBox)
            } else {
                // One line at its natural width, so the row's ideal width is what ViewThatFits
                // measures, never a silently truncated or hyphenated label.
                Label(title, systemImage: icon)
                    .lineLimit(1)
                    .fixedSize(horizontal: true, vertical: false)
            }
        }
        .buttonStyle(.bordered)
        .accessibilityLabel(title)
    }

    /// Whether the pane offers to load this message's blocked remote images: only over a body
    /// that has some, and only until the user says yes.
    private var remoteImagesOffered: Bool {
        guard let body = bodySnapshot, !body.pending, !body.loadError, !loadRemoteImages,
            let html = body.html, !html.isEmpty
        else { return false }
        return body.hasRemoteImages
    }

    /// The body area, on the page the core says a message is drawn on.
    ///
    /// The page is the same for every state of an open (the gap before the body lands, the
    /// spinner, a plain-text body, a load error) so a body arriving changes what is written on
    /// the page and never the page itself. Leaving the gap `Color.clear` punched a hole in it:
    /// against a dark appearance the body area went white, black, white on every message opened,
    /// which reads as a flicker rather than as a message opening (docs/sync-progress.md).
    ///
    /// The light appearance goes with the page rather than decorating it. The canvas is white in
    /// both themes because `base_css` pins the document to `color-scheme: light` (mail is authored
    /// for a white page) so the dark appearance's own label colours over it would be white on
    /// white. The chrome around it (header, toolbar, the remote-images banner) stays themed.
    @ViewBuilder
    private var content: some View {
        if remoteImagesOffered {
            RemoteImagesBanner { loadRemoteImages = true }
        }
        bodyArea
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(parseHexColor(messageCanvas().background))
            .environment(\.colorScheme, .light)
    }

    @ViewBuilder
    private var bodyArea: some View {
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
                SanitizedHTMLView(fragment: html, loadRemoteImages: loadRemoteImages)
            } else if let plain = body.plain, !plain.isEmpty {
                ScrollView {
                    Text(plain)
                        .font(.body)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(readingInset)
                }
            } else {
                placeholder(L10n.reading_no_content())
            }
        } else {
            // Opened, and nothing to say yet. Not a spinner: the body usually arrives within a
            // few milliseconds, and one drawn on every open flickers rather than reassures. The
            // header above is already filled from the row that was tapped, and the page is
            // already drawn, so the pane reads as the message opening rather than as empty.
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

/// What the reading pane shows while several rows are selected: the count, over whatever the pane
/// was holding. Opaque, so the message underneath is covered rather than shining through.
struct SelectionCountPane: View {
    let label: String

    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: "envelope.badge.fill")
                .font(.system(size: 32))
                .foregroundStyle(.tertiary)
            Text(label)
                .font(.title3)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(.background)
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
        .padding(.horizontal, readingInset)
        .padding(.vertical, 8)
        .background(.quaternary)
    }
}

