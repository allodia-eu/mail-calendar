import MailcalBindings
import SwiftUI

extension ContentView {
    /// `content` with the banners stacked above it, newest concern on top.
    ///
    /// On the Mac that is the window's whole content, under the toolbar. On iPhone and iPad it is
    /// each navigation stack's own content, under its bar: an inset outside a tab's or a column's
    /// stack does not move that stack's bar down, so a banner there covers the bar and its
    /// buttons.
    func bannered(_ content: some View) -> some View {
        content
            .safeAreaInset(edge: .top) { offlineBanner }
            .safeAreaInset(edge: .top) { mailReauthBanner }
            .safeAreaInset(edge: .top) { signInExpiredBanner }
            .safeAreaInset(edge: .top) { accountNoticeBanner }
            .safeAreaInset(edge: .top) { folderNoticeBanner }
    }

    /// A persistent banner shown while the device has no network, so the user knows the mail on
    /// screen is the last-synced copy (not stale silently). It clears itself the moment
    /// connectivity returns, when the core auto-refreshes.
    @ViewBuilder var offlineBanner: some View {
        if model.isOffline {
            HStack(spacing: 8) {
                Image(systemName: "wifi.slash").foregroundStyle(.secondary)
                Text(L10n.connectivity_offline_banner())
                    .font(.callout).fixedSize(horizontal: false, vertical: true)
                Spacer()
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 9)
            .bannerBackground()
            .overlay(alignment: .bottom) { Divider() }
        }
    }

    /// Shown on the mailbox when a Microsoft account's mail write/send is withheld for lack of the
    /// `Mail.ReadWrite` / `Mail.Send` OAuth scopes (connected before those scopes, or revoked
    /// consent), so a send or a mail action was refused with `403 ErrorAccessDenied`. Reading is
    /// unaffected, a permission prompt, not an outage, so it uses the informational style, like
    /// the calendar one. "Reconnect" signs that account in again in place for everything it is
    /// used for (`docs/provider-oauth.md` rule 11), which clears this and any calendar prompt; the
    /// account keeps its mail and settings.
    @ViewBuilder var mailReauthBanner: some View {
        let ids = model.connectivity?.mailReauthAccounts ?? []
        if let first = ids.first {
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                Text(L10n.mail_reauth_prompt(accounts: ids.map(model.accountEmail).joined(separator: ", ")))
                    .font(.callout).fixedSize(horizontal: false, vertical: true)
                Spacer()
                // One button signs the first affected account in again; when several are affected
                // the banner re-renders after each clears, walking through them one at a time.
                Button(L10n.mail_reauth_action()) { model.signInAgainFromBanner(first) }
                    .buttonStyle(.borderless)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 9)
            .bannerBackground()
            .overlay(alignment: .bottom) { Divider() }
        }
    }

    /// Shown when an account's stored sign-in has stopped being accepted, an expired or revoked
    /// OAuth grant (Google `invalid_grant`, a Microsoft `AADSTS700082`), or a refused password.
    /// Distinct from the two permission prompts above: nothing about the account syncs, and
    /// distinct from the offline/unreachable badges because the server *was* reached and a retry
    /// will never help. A Microsoft or Google account is signed in again in place, and a JMAP or
    /// IMAP account connected by signing in signs in again from its own persisted grant; a
    /// password or pasted-secret account opens its page in Settings, where the new one is entered.
    @ViewBuilder var signInExpiredBanner: some View {
        let accounts = model.signInExpiredAccounts
        if let first = accounts.first {
            let emails = accounts.map(\.email).joined(separator: ", ")
            let remedy = AccountSignInRules.remedy(first.provider)
            let browserFlow = remedy != .settings
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                Text(
                    browserFlow
                        ? L10n.signin_expired_prompt(accounts: emails)
                        : L10n.signin_expired_prompt_settings(accounts: emails)
                )
                .font(.callout).fixedSize(horizontal: false, vertical: true)
                Spacer()
                // One button signs the first affected account in again; with several affected the
                // banner re-renders after each clears, walking through them one at a time.
                switch remedy {
                case .consent:
                    Button(L10n.signin_expired_action()) { model.signInAgainFromBanner(first.id) }
                        .buttonStyle(.borderless)
                case .jmapReauth:
                    // Addressed to the account id, not the address: the core re-authorises that
                    // account's own stored grant rather than starting a discovery from an email.
                    Button(L10n.signin_expired_action()) {
                        Task { await model.reconnectJmap(accountId: first.id) }
                    }
                    .buttonStyle(.borderless)
                    .disabled(model.jmapReconnecting)
                case .accountSignIn:
                    // Addressed to the account id too, and signed in from its stored grant. Live
                    // while the browser is up, so a sign-in abandoned there never wedges it.
                    Button(L10n.signin_expired_action()) {
                        model.signInThroughProviderFromBanner(first.id)
                    }
                    .buttonStyle(.borderless)
                    .disabled(model.signInSwitchBusy == first.id)
                case .settings:
                    Button(L10n.settings_title()) { model.openAccountPage(first.id) }
                        .buttonStyle(.borderless)
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 9)
            .bannerBackground()
            .overlay(alignment: .bottom) { Divider() }
        }
    }

    /// A dismissible inline notice that some accounts were skipped at launch (their mail
    /// connect failed). Non-blocking: it sits above the mail list until the user clears it.
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
