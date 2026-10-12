// The manual form's panes, one per account kind, and what each submits. Split from
// AccountSetupView.swift, which holds the form around them.

import SwiftUI
import MailcalBindings

extension AccountSetupView {
    var canConnect: Bool {
        !imapHost.isEmpty && !username.isEmpty && !password.isEmpty
    }

    /// JMAP needs an email and the secret; the server may be left blank.
    var canConnectJmap: Bool {
        !username.isEmpty && !password.isEmpty
    }

    /// A calendar-and-contacts account needs its login and password; which server is checked on
    /// Connect, which marks the field when neither is filled.
    var canConnectDav: Bool {
        !username.trimmingCharacters(in: .whitespaces).isEmpty && !password.isEmpty
    }

    /// A password already typed counts as the route chosen: an answer arriving mid-typing must
    /// not take the field away.
    var passwordRouteChosen: Bool { passwordChosen || !password.isEmpty }

    /// The address a provider's sign-in targets (`login_hint`); left blank, it shows its picker.
    var typedHint: String? {
        let typed = username.trimmingCharacters(in: .whitespaces)
        return typed.isEmpty ? nil : typed
    }

    /// What a provider's sign-in asks for: the uses chosen on screen, less any the address typed
    /// is not offered, so a personal address edited in is never asked for colleagues.
    var providerChoice: [AccountCapability]? {
        SetupUses.allowed(providerUses.chosen.uses, providerOffer.choices)
    }

    // MARK: Panes

    /// The IMAP / password account fields.
    var imapForm: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.setup_credentials_note())
                .font(.caption).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            SetupCard(title: L10n.setup_section_account(), systemImage: "envelope") {
                serverRow(
                    placeholder: L10n.setup_imap_placeholder(), host: $imapHost,
                    field: $servers.imap)
                TextField(L10n.setup_field_email(), text: $username)
                    .setupField(.email)
                ImapAuthExplanation(state: imapAuth)
                if imapAuth.offersSignIn {
                    ImapSignInButton(
                        request: typedImapRequest,
                        signIn: { request in await model.signInWithImap(request) },
                        signingIn: $imapSigningIn,
                        leads: !passwordRouteChosen,
                        passwordInstead: imapAuth.offersPasswordInstead(chosen: passwordRouteChosen)
                            ? { passwordChosen = true } : nil,
                        failed: { imapAuth = .failed }
                    )
                    .disabled(model.isConnecting)
                }
                if imapAuth.showsPasswordField(chosen: passwordRouteChosen) {
                    SecureField(L10n.setup_field_password(), text: $password)
                        .setupField(.password)
                        .onTouchKeyboardSubmit {
                            guard canConnect, certificateOK, !model.isConnecting else { return }
                            submitImap()
                        }
                }
            }
            .task(id: ImapAuthQuestion(typedImapRequest)) {
                await askTypedImapServer()
            }
            SetupCard(title: L10n.setup_section_advanced(), systemImage: "slider.horizontal.3") {
                serverRow(
                    placeholder: L10n.setup_smtp_placeholder(), host: $smtpHost,
                    field: $servers.smtp)
                TextField(L10n.setup_caldav_placeholder(), text: $caldavURL)
                    .setupField(.host)
                // An address book of its own, for one that is not on the calendar's server.
                TextField(L10n.setup_field_carddav_optional(), text: $carddavURL)
                    .setupField(.host)
                // Directly under the fields it explains.
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
    var jmapForm: some View {
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
                        isAvailable: { email, server in
                            await model.jmapOAuthAvailable(email: email, serverURL: server)
                        },
                        offered: $jmapSignInOffered
                    )
                if jmapSignInOffered {
                    JmapSignInButton(email: username, serverURL: jmapServer) { email, server in
                        await model.signInWithJmap(email: email, serverURL: server)
                    }
                }
                SecureField(L10n.setup_jmap_secret_placeholder(), text: $password)
                    .setupField(.password)
                    .onTouchKeyboardSubmit {
                        guard canConnectJmap, !model.isConnecting else { return }
                        model.submitJmapSetup(email: username, serverURL: jmapServer, password: password)
                    }
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

    /// An account used for its calendar and contacts alone: the login, the calendar server, the
    /// address-book server, or both, and the password.
    var davForm: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.setup_dav_note())
                .font(.callout).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            Text(L10n.setup_credentials_note())
                .font(.caption).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            SetupCard(title: L10n.setup_section_account(), systemImage: "key") {
                TextField(L10n.setup_field_username(), text: $username)
                    .setupField(.email)
                SecureField(L10n.setup_field_password(), text: $password)
                    .setupField(.password)
                    .onTouchKeyboardSubmit {
                        guard canConnectDav, certificateOK, !model.isConnecting else { return }
                        submitDav()
                    }
            }
            SetupCard(title: L10n.setup_detect_section_calendar(), systemImage: "calendar") {
                davServerField(L10n.setup_field_caldav_optional(), text: $caldavURL)
                    .focused($calendarServerFocused)
            }
            SetupCard(title: L10n.setup_detect_section_contacts(), systemImage: "person.2") {
                davServerField(L10n.setup_field_carddav_optional(), text: $carddavURL)
            }
        }
    }

    /// One of the calendar-and-contacts servers, marked while Connect found neither filled.
    private func davServerField(_ placeholder: String, text: Binding<String>) -> some View {
        TextField(
            placeholder,
            text: Binding(
                get: { text.wrappedValue },
                set: {
                    text.wrappedValue = $0
                    flagged = .none
                }
            )
        )
        .setupField(.host)
        .overlay {
            if flagged != .none {
                RoundedRectangle(cornerRadius: 6).stroke(.red, lineWidth: 1.5)
            }
        }
    }

    /// The Microsoft 365 branch: what the sign-in does, the address it targets, and what the
    /// account is to be used for, which is all the sign-in asks for (`docs/accounts.md` rule 10).
    var microsoftForm: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.setup_microsoft_choice_note())
                .font(.callout).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            providerAddress
            SetupUsesSection(choices: $providerUses, flagged: .constant(.none)) { EmptyView() }
        }
        .padding(.vertical, 8)
    }

    /// The Google branch: the same, then the Early Access gate (a mandatory confirmation while
    /// Gmail is allow-listed). The "Sign in" button lives in the shared footer, disabled until the
    /// gate's toggle is on.
    var googleForm: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.setup_google_choice_note())
                .font(.callout).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            providerAddress
            SetupUsesSection(choices: $providerUses, flagged: .constant(.none)) { EmptyView() }
            GoogleEarlyAccessGate(confirmed: $googleEarlyAccessConfirmed)
        }
        .padding(.vertical, 8)
    }

    /// The address a provider's sign-in is for; left blank, the provider shows its own picker.
    private var providerAddress: some View {
        TextField(L10n.setup_field_email(), text: $username)
            .setupField(.email)
    }

    // MARK: Submits

    func submitImap() {
        model.submitSetup(
            imapHost: servers.imap.dial(imapHost), username: username, password: password,
            smtpHost: servers.smtp.dial(smtpHost), caldavBaseUrl: caldavURL,
            imapSecurity: servers.imap.security, smtpSecurity: servers.smtp.security,
            acceptedCertificate: acceptedCertificate,
            carddavBaseUrl: carddavURL.trimmingCharacters(in: .whitespaces))
    }

    /// Connects an account for its calendar and contacts alone. What it is used for follows from
    /// the servers typed; with neither, both fields are marked.
    func submitDav() {
        let (caldav, carddav) = (
            caldavURL.trimmingCharacters(in: .whitespaces),
            carddavURL.trimmingCharacters(in: .whitespaces)
        )
        guard let uses = SetupUses.davUses(caldav, carddav) else {
            flagged = .calendar
            calendarServerFocused = true
            return
        }
        model.submitSetup(
            imapHost: "", username: username.trimmingCharacters(in: .whitespaces),
            password: password, smtpHost: "", caldavBaseUrl: caldav,
            acceptedCertificate: acceptedCertificate, carddavBaseUrl: carddav, uses: uses)
    }

    /// The account the typed fields describe, as the pre-flight and the sign-in both take it.
    var typedImapRequest: ImapLoginRequest {
        imapLoginRequest(
            email: username,
            imapHost: servers.imap.dial(imapHost),
            smtpHost: smtpHost.isEmpty ? nil : servers.smtp.dial(smtpHost),
            caldavURL: caldavURL.isEmpty ? nil : caldavURL,
            // The port and security the person chose for each server, as the Connect button
            // submits them.
            imapSecurity: servers.imap.security,
            smtpSecurity: servers.smtp.security,
            // Nothing was detected, so no provider named an issuer for itself: the core's
            // well-known probe is what answers here.
            oauthIssuer: nil,
            carddavURL: carddavURL.isEmpty ? nil : carddavURL
        )
    }

    /// Asks again whenever the address or the server, its port and security included, changes;
    /// debounced so a network round trip does not go out per keystroke.
    func askTypedImapServer() async {
        imapAuth = .password
        passwordChosen = false
        guard kind == .imap, JmapOAuthProbe.looksLikeAddress(username), !imapHost.isEmpty else {
            return
        }
        try? await Task.sleep(for: JmapOAuthProbe.debounce)
        guard !Task.isCancelled else { return }
        let offer = await model.imapAuthOptions(typedImapRequest)
        // The person kept typing while the (blocking, uncancellable) call ran: its answer is
        // about a server they have already moved on from.
        guard !Task.isCancelled else { return }
        imapAuth = ImapAuthState(offer)
    }
}
