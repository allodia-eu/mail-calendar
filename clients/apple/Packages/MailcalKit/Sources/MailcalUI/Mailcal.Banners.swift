import MailcalBindings
import SwiftUI

extension ContentView {
    /// `content` with what stands above it: on iPhone the notice row (docs/notices.md, the Mac
    /// and the iPad put theirs in the sidebar), and a line about something an action just did.
    ///
    /// On the Mac that is the window's whole content, under the toolbar. On iPhone and iPad it is
    /// each navigation stack's own content, under its bar: an inset outside a tab's or a column's
    /// stack does not move that stack's bar down, so a row there covers the bar and its buttons.
    func bannered(_ content: some View) -> some View {
        #if os(macOS)
        // Stacked above the split view rather than inset into it: a safe-area inset around a
        // split view reaches the sidebar but not the detail column, so a line there covered the
        // mailbox's actions bar.
        VStack(spacing: 0) {
            accountNoticeBanner
            content
        }
        #else
        content
            .safeAreaInset(edge: .top) {
                if hSize == .compact {
                    NoticeStrip(model: model, destination: model.destination)
                }
            }
            .safeAreaInset(edge: .top) { accountNoticeBanner }
        #endif
    }

    /// A line about something an action just did, such as a sign-in from Settings that did not
    /// complete. Non-blocking: it sits above the list until the user closes it.
    @ViewBuilder var accountNoticeBanner: some View {
        if let notice = model.accountNotice {
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                Text(notice).font(.callout).fixedSize(horizontal: false, vertical: true)
                Spacer()
                Button(L10n.action_close()) { model.accountNotice = nil }
                    .buttonStyle(.borderless)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 9)
            .bannerBackground()
            .overlay(alignment: .bottom) { Divider() }
        }
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
