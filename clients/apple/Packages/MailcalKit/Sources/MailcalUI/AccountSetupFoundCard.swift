// The found card of the email-first setup flow: what detection found for the address, the uses
// it can set the account up for (docs/account-autodetect.md rule 8), and the one step left,
// signing in or connecting. Split from AccountSetupDetectView.swift, which holds the steps around
// it.

import SwiftUI
import MailcalBindings

extension AccountSetupDetectView {
    @ViewBuilder
    func foundView(_ setup: DetectedSetup) -> some View {
        var form = DetectedConnectForm(setup)
        let _ = {
            form.password = password
            form.approved = approved
            form.rejectedCertificate = model.setupRejectedCertificate
            form.certificateAccepted = certificateAccepted
        }()
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.setup_detect_found_title()).font(.title2).bold()

            if setup.calendarAndContacts {
                calendarAndContactsCard(form)
            } else {
                switch setup.recommendation {
                case let .microsoft(microsoftEmail):
                    providerCard(
                        hint: L10n.setup_detect_microsoft_hint(),
                        note: L10n.setup_microsoft_choice_note()
                    ) {
                        if model.microsoftSigningIn {
                            progress(L10n.setup_microsoft_signing_in())
                        } else {
                            Button(L10n.setup_microsoft_signin()) {
                                guard flaggedNothing() else { return }
                                model.signInWithMicrosoft(
                                    loginHint: microsoftEmail, uses: uses.chosen.uses)
                            }
                            .buttonStyle(.borderedProminent)
                            .keyboardShortcut(.defaultAction)
                        }
                    }
                case let .google(googleEmail):
                    providerCard(
                        hint: L10n.setup_detect_google_hint(),
                        note: L10n.setup_google_choice_note(),
                        // Same Early Access gate as the manual Google form: this path also
                        // reaches beginGoogleLogin, which Google blocks for anyone not yet
                        // allow-listed.
                        gate: { GoogleEarlyAccessGate(confirmed: $googleEarlyAccessConfirmed) }
                    ) {
                        if model.googleSigningIn {
                            progress(L10n.setup_google_signing_in())
                        } else {
                            Button(L10n.setup_google_signin()) {
                                guard flaggedNothing() else { return }
                                model.signInWithGoogle(loginHint: googleEmail, uses: uses.chosen.uses)
                            }
                            .buttonStyle(.borderedProminent)
                            .keyboardShortcut(.defaultAction)
                            .disabled(!googleEarlyAccessConfirmed)
                        }
                    }
                case let .jmap(jmapEmail, serverURL, _, _):
                    jmapCard(form, email: jmapEmail, serverURL: serverURL)
                case let .imap(
                    imapEmail, imapHost, smtpHost, imapSecurity, smtpSecurity, incoming, outgoing,
                    _, oauthIssuer, _, _
                ):
                    let request = imapLoginRequest(
                        email: imapEmail, imapHost: imapHost, smtpHost: smtpHost,
                        caldavURL: nonEmpty(uses.chosen.caldavURL), imapSecurity: imapSecurity,
                        smtpSecurity: smtpSecurity, oauthIssuer: oauthIssuer,
                        carddavURL: nonEmpty(uses.chosen.carddavURL), uses: uses.chosen.uses
                    )
                    imapCard(form, request: request, incoming: incoming, outgoing: outgoing)
                case .manual:
                    EmptyView() // never routed here
                }
            }

            Button(L10n.setup_detect_manual()) { phase = .manual(nil, setup.recommendation) }
        }
    }

    // MARK: Routes

    /// Microsoft and Google: one browser sign-in, no server details, asking for the uses chosen
    /// here alone (docs/accounts.md rule 10).
    private func providerCard<Gate: View, Action: View>(
        hint: String, note: String,
        @ViewBuilder gate: () -> Gate = { EmptyView() },
        @ViewBuilder action: @escaping () -> Action
    ) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(hint).font(.callout).fixedSize(horizontal: false, vertical: true)
            Text(note).font(.callout).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            SetupUsesSection(choices: $uses, flagged: $flagged) { EmptyView() }
            gate()
            // A failed or declined sign-in surfaces here, so the user isn't left on a silent
            // dead end and can retry or set up manually.
            inlineError()
            footer(action)
        }
    }

    private func jmapCard(_ form: DetectedConnectForm, email: String, serverURL: String) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.setup_detect_found_jmap_note()).font(.callout).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            SetupCard(title: L10n.setup_detect_section_email(), systemImage: "envelope") {
                if !serverURL.isEmpty {
                    detectedRow(protocolName: "JMAP", detail: urlHost(serverURL))
                }
                approvalControls(form)
                // Offered above the secret when this server advertises OAuth, and never instead
                // of it. An untrusted (non-HTTPS) result cannot reach here: the core's discovery
                // requires HTTPS at every hop and declines otherwise, so the button stays hidden.
                if jmapSignInOffered {
                    JmapSignInButton(email: email, serverURL: serverURL) { email, server in
                        await model.signInWithJmap(email: email, serverURL: server)
                    }
                }
                SecureField(L10n.setup_jmap_secret_placeholder(), text: $password)
                    .setupField(.password)
                    .onTouchKeyboardSubmit {
                        guard form.canConnect, !model.isConnecting else { return }
                        model.submitJmapSetup(email: email, serverURL: serverURL, password: password)
                    }
                    .jmapOAuthProbe(
                        email: email,
                        serverURL: serverURL,
                        isAvailable: { email, server in
                            await model.jmapOAuthAvailable(email: email, serverURL: server)
                        },
                        offered: $jmapSignInOffered
                    )
            }
            inlineError()
            footer {
                connectButton(enabled: form.canConnect) {
                    model.submitJmapSetup(email: email, serverURL: serverURL, password: password)
                }
            }
        }
    }

    private func imapCard(
        _ form: DetectedConnectForm, request: ImapLoginRequest, incoming: DetectedServerRow,
        outgoing: DetectedServerRow?
    ) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            SetupUsesSection(choices: $uses, flagged: $flagged) {
                serverRow(incoming)
                if let outgoing { serverRow(outgoing) }
            }
            SetupCard(title: L10n.setup_section_account(), systemImage: "key") {
                ImapAuthExplanation(state: imapAuth)
                approvalControls(form)
                if imapAuth.offersSignIn {
                    ImapSignInButton(
                        request: request,
                        signIn: { request in
                            guard flaggedNothing() else { return .cancelled }
                            return await model.signInWithImap(request)
                        },
                        signingIn: $imapSigningIn, leads: !passwordChosen,
                        passwordInstead: imapAuth.offersPasswordInstead(chosen: passwordChosen)
                            ? { passwordChosen = true } : nil,
                        failed: { imapAuth = .failed }
                    )
                    .disabled(!form.canSignIn || model.isConnecting)
                }
                if imapAuth.showsPasswordField(chosen: passwordChosen) {
                    Text(L10n.setup_detect_app_password_hint())
                        .font(.caption).foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                    SecureField(L10n.setup_field_password(), text: $password)
                        .setupField(.password)
                        .onTouchKeyboardSubmit {
                            guard form.canConnect, !imapSigningIn, !model.isConnecting else { return }
                            connectStandards(form, request: request)
                        }
                    certificateControls(form)
                }
            }
            .task(id: "\(request.email)|\(request.imapHost)") {
                await askImapServer(request)
            }
            inlineError(suppressed: form.refusedCertificate != nil)
            // Back and Cancel whatever the server said; Connect once a password field is shown.
            // A sign-in that leads is drawn in the card, under the line that explains it.
            footer {
                if imapAuth.showsPasswordField(chosen: passwordChosen) {
                    connectButton(enabled: form.canConnect && !imapSigningIn) {
                        connectStandards(form, request: request)
                    }
                }
            }
        }
    }

    /// No mail server was found for the address, but a calendar or address book was: the account
    /// is set up for those alone, the typed address as its login.
    private func calendarAndContactsCard(_ form: DetectedConnectForm) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.setup_detect_dav_note()).font(.callout).foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            SetupUsesSection(choices: $uses, flagged: $flagged) { EmptyView() }
            SetupCard(title: L10n.setup_section_account(), systemImage: "key") {
                Text(L10n.setup_credentials_note())
                    .font(.caption).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                SecureField(L10n.setup_field_password(), text: $password)
                    .setupField(.password)
                    .onTouchKeyboardSubmit {
                        guard form.canConnect, !model.isConnecting else { return }
                        connectStandards(form, request: nil)
                    }
                certificateControls(form)
            }
            inlineError(suppressed: form.refusedCertificate != nil)
            footer {
                connectButton(enabled: form.canConnect) {
                    connectStandards(form, request: nil)
                }
            }
        }
    }

    // MARK: Actions

    /// Whether every use switched on has its server. When one does not, its field is marked and
    /// focused, and the action stops there.
    private func flaggedNothing() -> Bool {
        flagged = uses.missing
        return flagged == .none
    }

    /// Connects a standards account: its mail servers when it is used for mail, and the calendar
    /// and address book the card found or was given.
    private func connectStandards(_ form: DetectedConnectForm, request: ImapLoginRequest?) {
        guard flaggedNothing() else { return }
        let chosen = uses.chosen
        model.submitSetup(
            imapHost: request?.imapHost ?? "",
            username: request?.email ?? email,
            password: password,
            smtpHost: request?.smtpHost ?? "",
            caldavBaseUrl: chosen.caldavURL,
            imapSecurity: request?.imapSecurity ?? .implicitTls,
            smtpSecurity: request?.smtpSecurity ?? .implicitTls,
            acceptedCertificate: form.acceptedCertificate,
            carddavBaseUrl: chosen.carddavURL,
            uses: chosen.uses
        )
    }

    /// Asks the mail server what it accepts. The card shows nothing to act on while it asks, so a
    /// server that never answers must not be able to hold somebody here: whichever of the answer
    /// and the deadline lands first decides, and the loser is dropped rather than rebuilding a card
    /// the person has started using (docs/mail-oauth.md rule 8).
    private func askImapServer(_ request: ImapLoginRequest) async {
        imapAuth = .checking
        passwordChosen = false
        let deadline = Task { @MainActor in
            try? await Task.sleep(for: ImapAuthState.deadline)
            if case .checking = imapAuth { imapAuth = .password }
        }
        let offer = await model.imapAuthOptions(
            imapLoginRequest(
                email: request.email, imapHost: request.imapHost, smtpHost: request.smtpHost,
                caldavURL: nil, imapSecurity: request.imapSecurity ?? .implicitTls,
                smtpSecurity: request.smtpSecurity ?? .implicitTls, oauthIssuer: request.oauthIssuer
            )
        )
        deadline.cancel()
        // The person may have edited the address while the (blocking, uncancellable) call ran;
        // `.task(id:)` has already restarted for the server they moved on to.
        guard !Task.isCancelled else { return }
        if case .checking = imapAuth { imapAuth = ImapAuthState(offer) }
    }

    // MARK: Small pieces

    /// The refused certificate and the confirmation that unlocks Connect, shown only once a
    /// connect has actually been refused for one.
    @ViewBuilder
    private func certificateControls(_ form: DetectedConnectForm) -> some View {
        if let refused = form.refusedCertificate {
            CertificateExceptionPanel(certificate: refused, accepted: $certificateAccepted)
        }
    }

    @ViewBuilder
    private func approvalControls(_ form: DetectedConnectForm) -> some View {
        if form.needsApproval {
            Text(L10n.setup_detect_untrusted_warning()).font(.caption).foregroundStyle(.red)
                .fixedSize(horizontal: false, vertical: true)
            Toggle(L10n.setup_detect_trust_confirm(), isOn: $approved)
        }
    }

    private func serverRow(_ row: DetectedServerRow) -> some View {
        detectedRow(
            protocolName: row.`protocol`,
            detail: "\(row.hostname):\(row.port) · \(row.security)"
        )
    }

    /// One discovered server, as a labelled row rather than a `·`-joined string: the protocol reads
    /// as the label it is, and the host/port/security line up down the card when there are two.
    private func detectedRow(protocolName: String, detail: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(protocolName)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
                .frame(width: 52, alignment: .leading)
            Text(detail)
                .font(.callout)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
    }

    /// The failure, in the transport's own words. Suppressed where a certificate panel is up:
    /// that panel says the same thing in the reader's language and with the certificate beside
    /// it, and the raw text under it is a second, worse copy of the question.
    @ViewBuilder
    private func inlineError(suppressed: Bool = false) -> some View {
        if let error = model.setupError, !suppressed {
            Text(error).font(.callout).foregroundStyle(.red)
        }
    }

    /// The found card's footer: Back at the start, and the same Cancel the other two steps carry
    /// when there is somewhere to cancel to.
    private func footer<Content: View>(@ViewBuilder _ content: @escaping () -> Content) -> some View {
        SetupFooter(back: stepBack, backDisabled: busy) {
            if let cancel {
                Button(L10n.action_cancel(), action: cancel)
                    .keyboardShortcut(.cancelAction)
            }
            content()
        }
    }

    /// A connect or a sign-in is running, and its answer belongs to the step on screen.
    private var busy: Bool {
        model.isConnecting || model.microsoftSigningIn || model.googleSigningIn || imapSigningIn
    }
}

private func nonEmpty(_ text: String) -> String? { text.isEmpty ? nil : text }
