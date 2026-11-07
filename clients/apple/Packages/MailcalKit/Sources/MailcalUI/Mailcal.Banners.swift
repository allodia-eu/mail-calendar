import MailcalBindings
import SwiftUI

extension ContentView {
    /// `content` with the notice row above it on iPhone, and on an iPad whose sidebar column, where
    /// its card is, is hidden (docs/notices.md); the Mac puts its own in the sidebar.
    ///
    /// Each navigation stack's own content, under its bar: an inset outside a tab's stack does not
    /// move that stack's bar down, so a row there covers the bar and its buttons.
    @ViewBuilder func bannered(_ content: some View) -> some View {
        #if os(macOS)
        content
        #else
        content.safeAreaInset(edge: .top) {
            if hSize == .compact || !model.sidebarShown {
                NoticeStrip(model: model, destination: model.destination)
            }
        }
        #endif
    }

    /// A transient hint above the mailbox: a spinner while a send is in flight, then a
    /// brief "Message sent" / "Couldn't send" confirmation. Driven by `model.sendStatus`,
    /// which the core updates and which auto-clears the terminal state.
    @ViewBuilder
    var sendStatusBanner: some View {
        switch model.sendStatus {
        case .idle:
            EmptyView()
        case .sending:
            sendBanner(L10n.send_status_sending(), systemImage: nil, tint: .secondary)
        case .sent:
            sendBanner(L10n.send_status_sent(), systemImage: "checkmark.circle.fill", tint: .green)
        case .sentNotFiled:
            // No transient hint: the standing UnfiledCopy question already says this, and says it with a button.
            EmptyView()
        case .queued:
            // Not a failure: the message is in the Outbox and goes out by itself. Saying it
            // failed would invite writing it a second time, and then both arrive.
            sendBanner(L10n.send_status_queued(), systemImage: "clock.fill", tint: .secondary)
        case .unconfirmed:
            // A warning, never a failure and never a success: the message may already be with
            // its recipients, and the Outbox row asks the user which it was.
            sendBanner(L10n.send_status_unconfirmed(), systemImage: "exclamationmark.triangle.fill", tint: .orange)
        case .notSent:
            // A failure that names where the message is: the server refused it, and the Outbox
            // row offers to send it again, edit it or discard it.
            sendBanner(L10n.send_status_not_sent(), systemImage: "exclamationmark.triangle.fill", tint: .orange)
        case .failed:
            // Never reached the Outbox: only the composer's draft keeps the message.
            sendBanner(L10n.send_status_failed(), systemImage: "exclamationmark.triangle.fill", tint: .orange)
        }
    }

    func sendBanner(_ text: String, systemImage: String?, tint: Color) -> some View {
        HStack(spacing: 8) {
            if let systemImage {
                Image(systemName: systemImage).foregroundStyle(tint)
            } else {
                ProgressView().controlSize(.small)
            }
            Text(text).font(.callout)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 9)
        .background(.thinMaterial, in: Capsule())
        .overlay(Capsule().strokeBorder(.quaternary))
        .shadow(radius: 6, y: 2)
        .padding(.top, 10)
        .transition(.move(edge: .top).combined(with: .opacity))
        .animation(.easeInOut(duration: 0.2), value: model.sendStatus)
    }
}

extension View {
    /// A banner's surface: a material, because the rows scroll on underneath it.
    ///
    /// ⚠️ Kept out of the top safe area, which a material fills by default: under a navigation bar
    /// with a large title it spreads over the title's place, and the title is not drawn.
    func bannerBackground() -> some View {
        background(.thinMaterial, ignoresSafeAreaEdges: .horizontal)
    }
}
