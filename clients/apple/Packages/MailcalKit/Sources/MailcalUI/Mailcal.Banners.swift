import MailcalBindings
import SwiftUI

extension ContentView {
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
            .background(.thinMaterial)
            .overlay(alignment: .bottom) { Divider() }
        }
    }

    /// Shown on the mailbox when a Microsoft account's mail write/send is withheld for lack of the
    /// `Mail.ReadWrite` / `Mail.Send` OAuth scopes (connected before those scopes, or revoked
    /// consent), so a send or a mail action was refused with `403 ErrorAccessDenied`. Reading is
    /// unaffected, a permission prompt, not an outage, so it uses the informational style, like
    /// the calendar one. "Reconnect" re-runs that account's sign-in with the full scope set
    /// (clearing this and any calendar prompt); the banner clears once a send/action succeeds.
    @ViewBuilder var mailReauthBanner: some View {
        let emails = model.mailReauthEmails
        if !emails.isEmpty {
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                Text(L10n.mail_reauth_prompt(accounts: emails.joined(separator: ", ")))
                    .font(.callout).fixedSize(horizontal: false, vertical: true)
                Spacer()
                // One button re-auths the first affected account; when several are affected the
                // banner re-renders after each clears, walking through them one sign-in at a time.
                Button(L10n.mail_reauth_action()) {
                    if let email = emails.first { model.signInWithMicrosoft(loginHint: email) }
                }
                .buttonStyle(.borderless)
                .disabled(model.microsoftSigningIn)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 9)
            .background(.thinMaterial)
            .overlay(alignment: .bottom) { Divider() }
        }
    }

    /// Shown when an account's stored sign-in has stopped being accepted, an expired or revoked
    /// OAuth grant (Google `invalid_grant`, a Microsoft `AADSTS700082`), or a refused password.
    /// Distinct from the two permission prompts above: nothing about the account syncs, and
    /// distinct from the offline/unreachable badges because the server *was* reached and a retry
    /// will never help. An OAuth account gets a button that re-runs its own sign-in, including a
    /// JMAP account connected by signing in, which re-authorises its own persisted grant; a
    /// password or pasted-secret JMAP account is pointed at Settings, since there is no browser
    /// flow to launch.
    @ViewBuilder var signInExpiredBanner: some View {
        let accounts = model.signInExpiredAccounts
        if let first = accounts.first {
            let emails = accounts.map(\.email).joined(separator: ", ")
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange)
                Text(
                    first.provider == .microsoft || first.provider == .google
                        || first.provider == .jmapOauth
                        ? L10n.signin_expired_prompt(accounts: emails)
                        : L10n.signin_expired_prompt_settings(accounts: emails)
                )
                .font(.callout).fixedSize(horizontal: false, vertical: true)
                Spacer()
                // One button re-signs-in the first affected account; with several affected the
                // banner re-renders after each clears, walking through them one at a time.
                switch first.provider {
                case .microsoft:
                    Button(L10n.signin_expired_action()) {
                        model.signInWithMicrosoft(loginHint: first.email)
                    }
                    .buttonStyle(.borderless)
                    .disabled(model.microsoftSigningIn)
                case .google:
                    Button(L10n.signin_expired_action()) {
                        model.signInWithGoogle(loginHint: first.email)
                    }
                    .buttonStyle(.borderless)
                    .disabled(model.googleSigningIn)
                case .jmapOauth:
                    // Addressed to the account id, not the address: the core re-authorises that
                    // account's own stored grant rather than starting a discovery from an email.
                    Button(L10n.signin_expired_action()) {
                        Task { await model.reconnectJmap(accountId: first.id) }
                    }
                    .buttonStyle(.borderless)
                    .disabled(model.jmapReconnecting)
                default:
                    EmptyView()
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 9)
            .background(.thinMaterial)
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
            .background(.thinMaterial)
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
        case .failed:
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
