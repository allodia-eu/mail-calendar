// The email-first account-setup flow: the user types only their email, the shared core
// detects their provider's settings, and we route them to a prefilled JMAP / IMAP / Microsoft
// path, falling back to the manual AccountSetupView (with a reason) when nothing usable is
// found. Mirrors the Android flow. The connect-gating (the untrusted-settings approval and the
// refused-certificate acceptance) lives in DetectedConnectForm next door, a plain struct the
// package test suite drives.

import SwiftUI
import MailcalBindings

struct AccountSetupDetectView: View {
    let error: String?
    /// The certificate the last connect was refused for, when that is why it failed. Drawn
    /// with the confirmation that unlocks Connect (`docs/certificate-exceptions.md`).
    var rejectedCertificate: RejectedCertificate? = nil
    var cancel: (() -> Void)? = nil
    let signInMicrosoft: (String?) -> Void
    let signInGoogle: (String?) -> Void
    var signingIn: Bool = false
    var googleSigningIn: Bool = false
    var connecting: Bool = false
    let submit: (String, String, String, String, String, ConnectionSecurity, ConnectionSecurity, RejectedCertificate?) -> Void
    let submitJmap: (String, String, String) -> Void
    /// Whether the detected JMAP server advertises OAuth sign-in (the blocking core pre-flight,
    /// run off the main thread), so the button is only offered where it works.
    let jmapOAuthAvailable: (String, String) async -> Bool
    /// Runs the JMAP browser sign-in and, on success, adds + stores the account.
    let signInJmap: (String, String) async -> JmapSignInOutcome
    /// Asks the mail server what it accepts, before any credential field is drawn.
    let imapAuthOptions: (ImapLoginRequest) async -> ImapAuthOffer
    /// Runs the IMAP browser sign-in and, on success, adds + stores the account.
    let signInImap: (ImapLoginRequest) async -> ImapSignInOutcome
    /// Runs the (blocking) core lookup; the caller hops off the main thread.
    let detect: (String) async -> SetupRecommendation
    /// The address an account offered by one of the person's other devices is for, filling the
    /// field.
    var startEmail: String = ""
    /// The whole record behind that address, when this flow was opened from an offer elsewhere.
    /// Its route is taken from what the other device wrote down rather than re-derived from the
    /// address, the round trip account sync exists to save.
    var startOffer: AllodiaAccountOffer?
    /// The Allodia onboarding block (`docs/onboarding.md`). Its card is first-run only; its offers
    /// are not.
    var onboarding: MailboxModel?
    /// Whether this is the screen somebody cannot skip.
    var firstRun = true
    /// Forgets what the abandoned step's attempt left on the model (its error, and a certificate it
    /// was refused or accepted) when the person steps back to the address.
    var clearAttempt: () -> Void = {}

    private enum Phase {
        case email
        case detecting
        case found(SetupRecommendation)
        case manual(MissReason?, SetupRecommendation?)

        /// Whether this is still the untouched first step, the showcase driver only ever fires
        /// from there, so a re-entrant `.task` can never restart a flow the user has moved on in.
        var isEmailStep: Bool {
            if case .email = self { return true }
            return false
        }
    }

    @State private var phase: Phase = .email
    @State private var email = ""
    @State private var password = ""
    @State private var approved = false
    /// Whether the person has accepted the certificate `rejectedCertificate` names. Reset
    /// whenever a different certificate arrives, so an acceptance never carries over to one
    /// nobody has looked at.
    @State private var certificateAccepted = false
    // nil = follow the detected default (on when a CalDAV endpoint was found); once the user
    // toggles, their choice sticks.
    @State private var calendarChoice: Bool?
    @State private var calendarURL = ""
    /// The mandatory Early Access confirmation for a detected Google account; sign-in stays
    /// disabled until it is on (same gate as the manual Google form).
    @State private var googleEarlyAccessConfirmed = false
    /// Whether the detected JMAP server advertises OAuth sign-in, as answered by `jmapOAuthProbe`.
    @State private var jmapSignInOffered = false
    /// What the detected IMAP server said it accepts. `.checking` until it answers, so the card
    /// draws no credential field in the meantime: one that appears and is then taken away reads
    /// as the app changing its mind (docs/mail-oauth.md rule 8).
    @State private var imapAuth: ImapAuthState = .checking

    var body: some View {
        // The manual form brings its own scaffold (it *is* AccountSetupView), so it is not wrapped
        // in a second one, nesting two would double the padding and the width cap.
        Group {
            if case let .manual(reason, edit) = phase {
                manualView(reason, edit)
            } else {
                SetupScaffold {
                    switch phase {
                    case .email: emailView
                    case .detecting: detectingView
                    case let .found(recommendation): foundView(recommendation)
                    case .manual: EmptyView() // handled above
                    }
                }
            }
        }
        // An offer only fills the field; it never skips a step. Guarded on the first step and on
        // an empty field, so a re-entrant appear cannot overwrite what somebody has typed.
        .onAppear {
            if phase.isEmailStep, email.isEmpty { email = startEmail }
            // An offer opened from elsewhere, the Settings list, lands on its own route, the
            // same as one pressed on this screen.
            if phase.isEmailStep, let startOffer { takeOffer(startOffer) }
        }
        .task { await driveShowcaseIfNeeded() }
        // A different certificate is a different decision, so an acceptance never carries
        // over to one nobody has been shown.
        .onChange(of: rejectedCertificate) { certificateAccepted = false }
    }

    /// Sets an offered account up on the route its record names, rather than re-deriving one from
    /// the address. That round trip is what syncing an account list exists to save, and for a
    /// domain that publishes no autoconfig it would find *less*, dropping the person onto the
    /// manual form for an account another device set up without trouble.
    ///
    /// The password is still asked for on this device, because no password travels.
    private func takeOffer(_ offer: AllodiaAccountOffer) {
        email = offer.email
        phase = route(setupFromOffer(offer: offer))
    }

    /// Documentation screenshots: type the seeded address and, for the later steps, run detection
    /// which in a showcase build answers instantly from a script keyed on the domain, so which
    /// screen this lands on is decided by the *core*, not by faking a phase here.
    ///
    /// Inert outside showcase mode, which is hard-`false` in a release build.
    private func driveShowcaseIfNeeded() async {
        guard ShowcaseMode.isOn, let seed = ShowcaseMode.setupSeed, phase.isEmailStep else { return }
        email = seed.email
        guard seed.runDetection else { return }
        phase = .detecting
        phase = route(await detect(email))
    }

    private var emailView: some View {
        VStack(alignment: .leading, spacing: 14) {
            // The mascot carries over from `WelcomeView`, at half its size: this is the very next
            // screen, and without it the step is three lines of text alone in the middle of an
            // iPad. Only this step gets it, the detected-settings and manual steps are dense
            // forms, where it would push the fields off screen for decoration.
            VStack(spacing: 10) {
                Image("WelcomeArt", bundle: .module)
                    .resizable()
                    .scaledToFit()
                    .frame(width: 72, height: 72)
                    .accessibilityHidden(true)
                Text(L10n.setup_detect_title())
                    .font(.title2).bold()
                    .multilineTextAlignment(.center)
                Text(L10n.setup_detect_description())
                    .font(.callout).foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity)
            .padding(.bottom, 4)

            // The recommendation, the way back for someone who already has an account, and the
            // divider that names what follows, above the address field, in that order
            // (`docs/onboarding.md`). Nothing at all in a build with no registration. On a later
            // add the card is gone and the offers are not: `firstRun` tells the two apart.
            if let onboarding {
                OnboardingAllodiaCard(model: onboarding, setUp: takeOffer, firstRun: firstRun)
            }

            TextField(L10n.setup_detect_email_placeholder(), text: $email)
                .setupField(.email)
            SetupFooter {
                if let cancel {
                    Button(L10n.action_cancel()) { cancel() }
                }
                Button(L10n.setup_detect_manual()) { phase = .manual(nil, nil) }
                Button(L10n.setup_detect_action()) {
                    Task {
                        phase = .detecting
                        phase = route(await detect(email))
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(email.isEmpty)
            }
        }
    }

    private var detectingView: some View {
        VStack(spacing: 16) {
            ProgressView()
            Text(L10n.setup_detect_looking()).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, minHeight: 160)
    }

    @ViewBuilder
    private func foundView(_ recommendation: SetupRecommendation) -> some View {
        var form = DetectedConnectForm(recommendation: recommendation)
        let calendarOn = calendarChoice ?? form.calendarEnabled
        let _ = {
            form.password = password
            form.approved = approved
            form.rejectedCertificate = rejectedCertificate
            form.certificateAccepted = certificateAccepted
            form.calendarEnabled = calendarOn
            form.calendarURLEntry = calendarURL
        }()
        VStack(alignment: .leading, spacing: 14) {
            Text(L10n.setup_detect_found_title()).font(.title2).bold()

            switch recommendation {
            case let .microsoft(microsoftEmail):
                Text(L10n.setup_detect_microsoft_hint()).font(.callout)
                    .fixedSize(horizontal: false, vertical: true)
                // A failed/declined sign-in surfaces as `error`; show it so the user isn't
                // left on a silent dead-end and can retry or set up manually.
                inlineError()
                footer {
                    if signingIn {
                        progress(L10n.setup_microsoft_signing_in())
                    } else {
                        Button(L10n.setup_microsoft_signin()) { signInMicrosoft(microsoftEmail) }
                            .buttonStyle(.borderedProminent)
                    }
                }
            case let .google(googleEmail):
                Text(L10n.setup_detect_google_hint()).font(.callout)
                    .fixedSize(horizontal: false, vertical: true)
                // Same Early Access gate as the manual Google form: this path also reaches
                // beginGoogleLogin, which Google blocks for anyone not yet allow-listed.
                GoogleEarlyAccessGate(confirmed: $googleEarlyAccessConfirmed)
                inlineError()
                footer {
                    if googleSigningIn {
                        progress(L10n.setup_google_signing_in())
                    } else {
                        Button(L10n.setup_google_signin()) { signInGoogle(googleEmail) }
                            .buttonStyle(.borderedProminent)
                            .disabled(!googleEarlyAccessConfirmed)
                    }
                }
            case let .jmap(jmapEmail, serverURL, _, _):
                Text(L10n.setup_detect_found_jmap_note()).font(.callout).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                SetupCard(title: L10n.setup_detect_section_email(), systemImage: "envelope") {
                    if !serverURL.isEmpty {
                        detectedRow(protocolName: "JMAP", detail: urlHost(serverURL))
                    }
                    approvalControls(form)
                    // Offered above the secret when this server advertises OAuth, and never
                    // instead of it. An untrusted (non-HTTPS) result cannot reach here: the core's
                    // discovery requires HTTPS at every hop and declines otherwise, so the button
                    // stays hidden.
                    if jmapSignInOffered {
                        JmapSignInButton(email: jmapEmail, serverURL: serverURL, signIn: signInJmap)
                    }
                    SecureField(L10n.setup_jmap_secret_placeholder(), text: $password)
                        .setupField(.password)
                        .jmapOAuthProbe(
                            email: jmapEmail,
                            serverURL: serverURL,
                            isAvailable: jmapOAuthAvailable,
                            offered: $jmapSignInOffered
                        )
                }
                inlineError()
                footer {
                    connectButton(enabled: form.canConnect) {
                        submitJmap(jmapEmail, serverURL, password)
                    }
                }
            case let .imap(
                imapEmail, imapHost, smtpHost, imapSecurity, smtpSecurity, incoming, outgoing,
                caldavURL, oauthIssuer, _, _
            ):
                SetupCard(title: L10n.setup_detect_section_email(), systemImage: "envelope") {
                    serverRow(incoming)
                    if let outgoing { serverRow(outgoing) }
                    ImapAuthExplanation(state: imapAuth)
                    approvalControls(form)
                    if imapAuth.offersSignIn {
                        ImapSignInButton(
                            request: imapLoginRequest(
                                email: imapEmail, imapHost: imapHost, smtpHost: smtpHost,
                                caldavURL: form.effectiveCaldavURL, imapSecurity: imapSecurity,
                                smtpSecurity: smtpSecurity, oauthIssuer: oauthIssuer
                            ),
                            signIn: signInImap,
                            failed: { imapAuth = .failed }
                        )
                    }
                    if imapAuth.showsPassword {
                        Text(L10n.setup_detect_app_password_hint())
                            .font(.caption).foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                        SecureField(L10n.setup_field_password(), text: $password)
                            .setupField(.password)
                        certificateControls(form)
                    }
                }
                .task(id: "\(imapEmail)|\(imapHost)") {
                    imapAuth = .checking
                    // The card shows nothing to act on while it asks, so a server that never
                    // answers must not be able to hold somebody here. Whichever answer lands
                    // first decides: both apply themselves only while the state is still
                    // `.checking`, so the loser is dropped rather than rebuilding a card the
                    // person has started using (docs/mail-oauth.md rule 8).
                    let deadline = Task { @MainActor in
                        try? await Task.sleep(for: ImapAuthState.deadline)
                        if case .checking = imapAuth { imapAuth = .password }
                    }
                    let offer = await imapAuthOptions(
                        imapLoginRequest(
                            email: imapEmail, imapHost: imapHost, smtpHost: smtpHost,
                            caldavURL: nil, imapSecurity: imapSecurity,
                            smtpSecurity: smtpSecurity, oauthIssuer: oauthIssuer
                        )
                    )
                    deadline.cancel()
                    // The person may have edited the address while the (blocking,
                    // uncancellable) call ran; `.task(id:)` has already restarted for the
                    // server they moved on to.
                    guard !Task.isCancelled else { return }
                    if case .checking = imapAuth { imapAuth = ImapAuthState(offer) }
                }
                calendarSection(discovered: caldavURL)
                inlineError(suppressed: form.refusedCertificate != nil)
                if imapAuth.showsPassword {
                    footer {
                        connectButton(enabled: form.canConnect) {
                            submit(imapHost, imapEmail, password, smtpHost ?? "", form.effectiveCaldavURL ?? "", imapSecurity, smtpSecurity, form.acceptedCertificate)
                        }
                    }
                }
            case .manual:
                EmptyView() // never routed here
            }

            Button(L10n.setup_detect_manual()) { phase = .manual(nil, recommendation) }
        }
    }

    @ViewBuilder
    private func manualView(_ reason: MissReason?, _ edit: SetupRecommendation?) -> some View {
        let prefill = manualPrefill(edit, typedEmail: email)
        AccountSetupView(
            error: error,
            rejectedCertificate: rejectedCertificate,
            cancel: cancel,
            back: stepBack,
            signInMicrosoft: signInMicrosoft,
            signInGoogle: signInGoogle,
            signingIn: signingIn,
            googleSigningIn: googleSigningIn,
            connecting: connecting,
            submit: submit,
            submitJmap: submitJmap,
            jmapOAuthAvailable: jmapOAuthAvailable,
            signInJmap: signInJmap,
            imapAuthOptions: imapAuthOptions,
            signInImap: signInImap,
            initialKind: prefill.kind,
            prefillEmail: prefill.email,
            prefillImapHost: prefill.imapHost,
            prefillSmtpHost: prefill.smtpHost,
            prefillJmapServer: prefill.jmapServer,
            note: reason.map(reasonNote)
        )
    }

    // MARK: - Small pieces

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

    /// The Calendar section of the found card. When detection discovered a CalDAV endpoint
    /// the toggle is pre-checked (opt-out) and its host shown; otherwise it's an opt-in
    /// toggle revealing a manual CalDAV field. Calendar reuses the IMAP credentials.
    @ViewBuilder
    private func calendarSection(discovered: String?) -> some View {
        let isOn = calendarChoice ?? (discovered != nil)
        SetupCard(title: L10n.setup_detect_section_calendar(), systemImage: "calendar") {
            Toggle(
                discovered != nil ? L10n.setup_detect_calendar_enable() : L10n.setup_detect_calendar_add(),
                isOn: Binding(get: { isOn }, set: { calendarChoice = $0 })
            )
            if isOn {
                if let discovered {
                    Text(urlHost(discovered)).font(.caption).foregroundStyle(.secondary)
                } else {
                    TextField(L10n.setup_hint_caldav(), text: $calendarURL).setupField(.host)
                }
            }
        }
    }

    /// The failure, in the transport's own words. Suppressed where a certificate panel is up:
    /// that panel says the same thing in the reader's language and with the certificate beside
    /// it, and the raw text under it is a second, worse copy of the question.
    @ViewBuilder private func inlineError(suppressed: Bool = false) -> some View {
        if let error, !suppressed {
            Text(error).font(.callout).foregroundStyle(.red)
        }
    }

    private func connectButton(enabled: Bool, action: @escaping () -> Void) -> some View {
        Group {
            if connecting {
                progress(L10n.status_connecting())
            } else {
                Button(L10n.action_connect(), action: action)
                    .buttonStyle(.borderedProminent)
                    .disabled(!enabled)
            }
        }
    }

    /// The found card's footer: Back at the start, and the same Cancel the other two steps carry
    /// when this is a later add.
    private func footer<Content: View>(@ViewBuilder _ content: @escaping () -> Content) -> some View {
        SetupFooter(back: stepBack, backDisabled: busy) {
            if let cancel {
                Button(L10n.action_cancel()) { cancel() }
            }
            content()
        }
    }

    /// A connect or a sign-in is running, and its answer belongs to the step on screen.
    private var busy: Bool { connecting || signingIn || googleSigningIn }

    /// Back to the address, which keeps what was typed there. Everything the abandoned route
    /// filled in goes with it: a different address can reach a different server, and nothing is
    /// accepted that was not shown.
    private func stepBack() {
        clearAttempt()
        password = ""
        approved = false
        certificateAccepted = false
        calendarChoice = nil
        calendarURL = ""
        googleEarlyAccessConfirmed = false
        jmapSignInOffered = false
        phase = .email
    }

    private func route(_ recommendation: SetupRecommendation) -> Phase {
        if case let .manual(reason) = recommendation {
            return .manual(reason, nil)
        }
        return .found(recommendation)
    }
}
