// The first-run account-setup form: pick an account type, then either enter IMAP details
// (mail server + email + password; SMTP/calendar optional, the core assumes the standard
// secure port and derives the TLS name from the host) or sign in to Microsoft 365 in the
// browser. On IMAP submit the model serializes the fields, stores them in the Keychain, and
// connects; the Microsoft button runs the browser OAuth flow. Split out of Mailcal.swift
// to keep each file under the 500-line limit.

import SwiftUI
import MailcalBindings

/// The account kinds the setup form can offer.
enum AccountKind: Hashable {
    case imap
    case microsoft
    case google
    case jmap

    /// The kinds this build offers, in picker order. A browser sign-in needs the provider's
    /// OAuth client registration, which is injected at build time, so a build given none drops
    /// the route rather than showing a button that fails at the provider. The two credential
    /// routes are always there.
    static var offered: [AccountKind] {
        let routes = oauthRoutes()
        return [.imap, .jmap, .microsoft, .google].filter { kind in
            switch kind {
            case .microsoft: return routes.microsoft
            case .google: return routes.google
            case .imap, .jmap: return true
            }
        }
    }

    var label: String {
        switch self {
        case .imap: return L10n.setup_account_type_password()
        case .jmap: return L10n.setup_account_type_jmap()
        case .microsoft: return L10n.setup_account_type_microsoft()
        case .google: return L10n.setup_account_type_google()
        }
    }
}

/// A setup form for a new account. Required (IMAP): mail server, email, password. Optional:
/// outgoing (SMTP) server, CalDAV URL. `submit` receives the IMAP fields (optionals empty
/// when unset); `signInMicrosoft` starts the Microsoft browser flow; `error` shows a failed
/// build/connect.
struct AccountSetupView: View {
    let error: String?
    /// Shown only when adding another account (the form as a sheet); `nil` on first run.
    var cancel: (() -> Void)? = nil
    /// Starts the Microsoft 365 sign-in (browser OAuth); the model handles the redirect.
    let signInMicrosoft: (String?) -> Void
    /// Starts the Google sign-in (browser OAuth); the model handles the redirect. Gated on the
    /// Early Access confirmation below.
    let signInGoogle: (String?) -> Void
    /// `true` while a Microsoft sign-in is running, so the button shows progress.
    var signingIn: Bool = false
    /// `true` while a Google sign-in is running, so the button shows progress.
    var googleSigningIn: Bool = false
    /// `true` while an IMAP Connect is running, so its button shows progress and is disabled.
    var connecting: Bool = false
    let submit:
        (
            _ imapHost: String, _ username: String, _ password: String,
            _ smtpHost: String, _ caldavURL: String,
            _ imapSecurity: ConnectionSecurity, _ smtpSecurity: ConnectionSecurity
        ) -> Void
    /// Serializes the JMAP fields, stores them, and connects (server may be empty:
    /// the core derives it from the email domain). One secret, whichever kind it is: the
    /// engine negotiates Basic vs. Bearer from the server's own challenge.
    let submitJmap:
        (
            _ email: String, _ serverURL: String, _ password: String
        ) -> Void
    /// Whether this JMAP server advertises OAuth sign-in, the blocking core pre-flight, run off
    /// the main thread. Decides whether the sign-in button is shown at all.
    let jmapOAuthAvailable: (_ email: String, _ serverURL: String) async -> Bool
    /// Runs the JMAP browser sign-in and, on success, adds + stores the account.
    let signInJmap: (_ email: String, _ serverURL: String) async -> JmapSignInOutcome
    /// Shown when the email-first detection flow routed here (nothing found, offline, an
    /// unsupported provider), so the user knows why they're entering settings by hand.
    var note: String? = nil

    @State private var kind: AccountKind
    /// What the typed mail server said it accepts. `.password` until it answers, because this
    /// pane's password field is already on screen and must not be taken away to say nothing.
    @State private var imapAuth: ImapAuthState = .password
    /// Asks the mail server what it accepts. `nil` where there is no core to ask, which is a
    /// preview or a test: the pane then simply never offers a sign-in.
    let imapAuthOptions: ((ImapLoginRequest) async -> ImapAuthOffer)?
    /// Runs the IMAP browser sign-in and, on success, adds + stores the account.
    let signInImap: ((ImapLoginRequest) async -> ImapSignInOutcome)?

    @State private var imapHost: String
    @State private var username: String
    @State private var password = ""
    @State private var smtpHost: String
    @State private var caldavURL = ""
    @State private var jmapServer: String
    /// The mandatory Early Access confirmation for Google; the "Sign in with Google" button stays
    /// disabled until it is on (Gmail is allow-listed while Google verifies the app).
    @State private var googleEarlyAccessConfirmed = false
    /// Whether the typed JMAP server advertises OAuth sign-in, as answered by `jmapOAuthProbe`.
    /// False until the core says otherwise, so the button is never offered on a guess.
    @State private var jmapSignInOffered = false

    /// The manual form, optionally prefilled from a detection result the user chose to edit
    /// (and with a `note` explaining why detection routed here).
    init(
        error: String?,
        cancel: (() -> Void)? = nil,
        signInMicrosoft: @escaping (String?) -> Void,
        signInGoogle: @escaping (String?) -> Void,
        signingIn: Bool = false,
        googleSigningIn: Bool = false,
        connecting: Bool = false,
        submit: @escaping (String, String, String, String, String, ConnectionSecurity, ConnectionSecurity) -> Void,
        submitJmap: @escaping (String, String, String) -> Void,
        jmapOAuthAvailable: @escaping (String, String) async -> Bool,
        signInJmap: @escaping (String, String) async -> JmapSignInOutcome,
        imapAuthOptions: ((ImapLoginRequest) async -> ImapAuthOffer)? = nil,
        signInImap: ((ImapLoginRequest) async -> ImapSignInOutcome)? = nil,
        initialKind: AccountKind = .imap,
        prefillEmail: String = "",
        prefillImapHost: String = "",
        prefillSmtpHost: String = "",
        prefillJmapServer: String = "",
        note: String? = nil
    ) {
        self.error = error
        self.cancel = cancel
        self.signInMicrosoft = signInMicrosoft
        self.signInGoogle = signInGoogle
        self.signingIn = signingIn
        self.googleSigningIn = googleSigningIn
        self.connecting = connecting
        self.submit = submit
        self.submitJmap = submitJmap
        self.jmapOAuthAvailable = jmapOAuthAvailable
        self.signInJmap = signInJmap
        self.imapAuthOptions = imapAuthOptions
        self.signInImap = signInImap
        self.note = note
        _kind = State(initialValue: initialKind)
        _imapHost = State(initialValue: prefillImapHost)
        _username = State(initialValue: prefillEmail)
        _smtpHost = State(initialValue: prefillSmtpHost)
        _jmapServer = State(initialValue: prefillJmapServer)
    }

    private var canConnect: Bool {
        !imapHost.isEmpty && !username.isEmpty && !password.isEmpty
    }

    /// JMAP needs an email and the secret; the server may be left blank.
    private var canConnectJmap: Bool {
        !username.isEmpty && !password.isEmpty
    }

    var body: some View {
        SetupScaffold {
            Text(L10n.setup_title()).font(.title2).bold()

            if let note {
                Text(note).font(.callout).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            Picker(L10n.setup_account_type(), selection: $kind) {
                ForEach(AccountKind.offered, id: \.self) { offered in
                    Text(offered.label).tag(offered)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()

            switch kind {
            case .imap: imapForm
            case .jmap: jmapForm
            case .microsoft: microsoftForm
            case .google: googleForm
            }

            if let error {
                Text(error).font(.callout).foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }

            SetupFooter {
                if let cancel {
                    Button(L10n.action_cancel()) { cancel() }
                }
                primaryAction
            }
        }
    }

    /// The footer's leading action, which is whichever verb the selected account kind needs:
    /// Connect for the two credential routes, Sign in for the two OAuth ones, or the progress
    /// readout that replaces it while one is running.
    @ViewBuilder private var primaryAction: some View {
        switch kind {
        case .imap:
            if connecting {
                connectingLabel(L10n.status_connecting())
            } else {
                Button(L10n.action_connect()) {
                    // The manual form only offers implicit-TLS setup today (STARTTLS
                    // arrives via autodetection).
                    submit(imapHost, username, password, smtpHost, caldavURL, .implicitTls, .implicitTls)
                }
                .buttonStyle(.borderedProminent)
                .disabled(!canConnect)
            }
        case .jmap:
            if connecting {
                connectingLabel(L10n.status_connecting())
            } else {
                Button(L10n.action_connect()) {
                    submitJmap(username, jmapServer, password)
                }
                .buttonStyle(.borderedProminent)
                .disabled(!canConnectJmap)
            }
        case .microsoft:
            if signingIn {
                connectingLabel(L10n.setup_microsoft_signing_in())
            } else {
                Button(L10n.setup_microsoft_signin()) { signInMicrosoft(username.isEmpty ? nil : username) }
                    .buttonStyle(.borderedProminent)
            }
        case .google:
            if googleSigningIn {
                connectingLabel(L10n.setup_google_signing_in())
            } else {
                // Disabled until the Early Access confirmation is on (see googleForm).
                Button(L10n.setup_google_signin()) { signInGoogle(username.isEmpty ? nil : username) }
                    .buttonStyle(.borderedProminent)
                    .disabled(!googleEarlyAccessConfirmed)
            }
        }
    }

    private func connectingLabel(_ label: String) -> some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text(label).foregroundStyle(.secondary)
        }
    }

    /// The IMAP / password account fields.
    private var imapForm: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.setup_credentials_note())
                .font(.caption).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            SetupCard(title: L10n.setup_section_account(), systemImage: "envelope") {
                TextField(L10n.setup_imap_placeholder(), text: $imapHost)
                    .setupField(.host)
                TextField(L10n.setup_field_email(), text: $username)
                    .setupField(.email)
                if imapAuth.offersSignIn || imapAuth.explainsRegistration {
                    ImapAuthExplanation(state: imapAuth)
                }
                if imapAuth.offersSignIn, let signInImap {
                    ImapSignInButton(
                        request: typedImapRequest,
                        signIn: signInImap,
                        failed: { imapAuth = .failed }
                    )
                }
                SecureField(L10n.setup_field_password(), text: $password)
                    .setupField(.password)
            }
            .task(id: "\(username)|\(imapHost)") {
                await askTypedImapServer()
            }
            SetupCard(title: L10n.setup_section_advanced(), systemImage: "slider.horizontal.3") {
                TextField(L10n.setup_smtp_placeholder(), text: $smtpHost)
                    .setupField(.host)
                TextField(L10n.setup_caldav_placeholder(), text: $caldavURL)
                    .setupField(.host)
                // Directly under the fields it explains. It used to sit after the whole `Form`,
                // which on iPad put it a screen-height below them.
                Text(L10n.setup_port_note())
                    .font(.caption2).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    /// The JMAP account fields: email + one secret (password *or* API token, the same box,
    /// since the core stores either the same way) + an optional server URL. When the server
    /// advertises OAuth, a "Sign in with your provider" button appears above the secret, an
    /// addition, never a replacement: discovery may decline on any server, so the field stays.
    /// Reuses the shared `username`/`password` state (only one kind is active at a time).
    private var jmapForm: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.setup_jmap_note())
                .font(.caption).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            SetupCard(title: L10n.setup_section_account(), systemImage: "envelope") {
                TextField(L10n.setup_field_email(), text: $username)
                    .setupField(.email)
                    .jmapOAuthProbe(
                        email: username,
                        serverURL: jmapServer,
                        isAvailable: jmapOAuthAvailable,
                        offered: $jmapSignInOffered
                    )
                if jmapSignInOffered {
                    JmapSignInButton(email: username, serverURL: jmapServer, signIn: signInJmap)
                }
                SecureField(L10n.setup_jmap_secret_placeholder(), text: $password)
                    .setupField(.password)
                Text(L10n.setup_jmap_secret_note())
                    .font(.caption2).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            SetupCard(title: L10n.setup_section_advanced(), systemImage: "slider.horizontal.3") {
                TextField(L10n.setup_jmap_server_placeholder(), text: $jmapServer)
                    .setupField(.host)
            }
        }
    }

    /// The Microsoft 365 branch: a short note, then the "Sign in" button lives in the shared
    /// footer so its placement matches the IMAP Connect button.
    private var microsoftForm: some View {
        Text(L10n.setup_microsoft_note())
            .font(.callout).foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.vertical, 8)
    }

    /// The Google branch: a short note, then the Early Access gate (a mandatory confirmation while
    /// Gmail is allow-listed). The "Sign in" button lives in the shared footer, disabled until the
    /// gate's toggle is on, so its placement matches the IMAP Connect / Microsoft Sign in buttons.
    private var googleForm: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.setup_google_note())
                .font(.callout).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
            GoogleEarlyAccessGate(confirmed: $googleEarlyAccessConfirmed)
        }
        .padding(.vertical, 8)
    }
}

extension AccountSetupView {
    /// The account the typed fields describe, as the pre-flight and the sign-in both take it.
    var typedImapRequest: ImapLoginRequest {
        imapLoginRequest(
            email: username,
            imapHost: imapHost,
            smtpHost: smtpHost.isEmpty ? nil : smtpHost,
            caldavURL: caldavURL.isEmpty ? nil : caldavURL,
            // The manual form is implicit-TLS only; a STARTTLS server arrives through
            // autodetection (docs/account-autodetect.md → Known gaps).
            imapSecurity: .implicitTls,
            smtpSecurity: .implicitTls,
            // Nothing was detected, so no provider named an issuer for itself: the core's
            // well-known probe is what answers here.
            oauthIssuer: nil
        )
    }

    /// Asks again whenever the address or the server changes, debounced so a network round trip
    /// does not go out per keystroke.
    func askTypedImapServer() async {
        imapAuth = .password
        guard let imapAuthOptions, kind == .imap,
              JmapOAuthProbe.looksLikeAddress(username), !imapHost.isEmpty
        else { return }
        try? await Task.sleep(for: JmapOAuthProbe.debounce)
        guard !Task.isCancelled else { return }
        let offer = await imapAuthOptions(typedImapRequest)
        // The person kept typing while the (blocking, uncancellable) call ran: its answer is
        // about a server they have already moved on from.
        guard !Task.isCancelled else { return }
        imapAuth = ImapAuthState(offer)
    }
}
