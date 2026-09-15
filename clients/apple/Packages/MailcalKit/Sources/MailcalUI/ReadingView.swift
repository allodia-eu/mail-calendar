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
/// Roughly four rows. On macOS the bar sits above the message in a column that does not scroll, so
/// this is the line between "this message has attachments" and "this message is now unreadable".
/// On iOS and iPadOS the bar goes up with the header (ReadingView.Scroll.swift) and the cap is
/// what keeps the message itself on the first screen rather than below twenty rows of files.
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

/// The reading view: a header plus the fetched body, or a spinner until the body for this
/// message arrives (the fetch is async, a network round-trip on the first open).
///
/// **One view, three hosts.** It is the third pane beside the message list (sidebar | list |
/// reading), the screen an iPhone pushes, and the whole of a detached reading window on the
/// desktop (`docs/reading-window.md`). What differs between them is where the body comes from
/// (`source`) and what the actions do, both handed in; nothing here knows which host it is in,
/// which is what keeps a window from drifting away from the pane it was opened out of.
///
/// A fresh instance is created per opened message (keyed by the row), so its per-message state
/// resets on selection.
struct ReadingView: View {
    var model: MailboxModel
    let message: OpenedMessage
    /// Which of the core's reading slots this view's body arrives in: the pane's, or one
    /// detached window's. Two views can be on screen at once and each must read only its own.
    var source: ReadingSource = .pane
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
    ///
    /// Not `private`: ReadingView.Scroll.swift draws the banner that sets it.
    @State var loadRemoteImages = false
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
        let slot: ReadingSnapshot? = switch source {
        case .pane: model.reading
        case .window(let id): model.readingWindows[id]?.body
        }
        guard let slot, slot.key == message.key else { return nil }
        return slot
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
        #if os(iOS)
        // Everything but the action row belongs to the message and scrolls with it
        // (ReadingView.Scroll.swift).
        VStack(spacing: 0) {
            actionToolbar
            Divider()
            content
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        #else
        VStack(spacing: 0) {
            identityHeader
            recipientsHeader
            actionToolbar
            attachmentBar
            invitationCard
            Divider()
            content
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        #endif
    }

    /// Who wrote this, what it is called and when it arrived: the one part of the header that is
    /// filled from the row the reader tapped, so it is complete before the body has been fetched.
    ///
    /// Not `private`: ReadingView.Scroll.swift assembles the scrolling header from it.
    var identityHeader: some View {
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
    }

    /// The meeting-invitation card, empty unless the core's RSVP gate called this message an
    /// invitation. Not `private`, see [`identityHeader`].
    var invitationCard: some View {
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
    }

    /// The recipient headers (To / Cc / Bcc), shown once this message's snapshot arrives.
    /// Each row appears only when non-empty; Bcc is present only on the user's own Sent/Drafts
    /// copies (whose stored message carries a Bcc header), so they can see whom they Bcc'd.
    ///
    /// Not `private`, see [`identityHeader`].
    @ViewBuilder
    var recipientsHeader: some View {
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
