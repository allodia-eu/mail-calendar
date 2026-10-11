// The email-first account-setup flow: the user types only their email, the shared core detects
// their provider's settings and which of mail, calendar and contacts it can set up, and we route
// them to a prefilled card (AccountSetupFoundCard.swift), falling back to the manual
// AccountSetupView (with a reason) when nothing usable is found. The connect-gating (the
// untrusted-settings approval and the refused-certificate acceptance) lives in
// DetectedConnectForm next door, and what the card's toggles make of the account in SetupUses,
// both plain structs the package test suite drives.

import SwiftUI
import MailcalBindings

/// The setup screen: the link step once an account has been added and another can fill
/// something it lacks (docs/onboarding.md, "After the connect"), the email-first flow otherwise.
/// One view for the first run and a later add, so the two cannot drift.
struct AccountSetupScreen: View {
    var model: MailboxModel
    /// Whether this is the first account, which offers the Allodia recommendation.
    var firstRun: Bool

    var body: some View {
        if model.setupFlow.links != nil {
            SetupLinkStepView(model: model)
        } else {
            // A fresh view whenever the link step hands back to it, so "Add another account"
            // starts at the address rather than on the card the last account was added from.
            AccountSetupDetectView(model: model, firstRun: firstRun)
        }
    }
}

struct AccountSetupDetectView: View {
    var model: MailboxModel
    /// Whether this is the screen somebody cannot skip: the first account, which is also the
    /// only place the Allodia card is drawn. Its offers are drawn on a later add too.
    var firstRun = true

    enum Phase {
        case email
        case detecting
        case found(DetectedSetup)
        case manual(MissReason?, SetupRecommendation?)

        /// Whether this is still the untouched first step, the showcase driver only ever fires
        /// from there, so a re-entrant `.task` can never restart a flow the user has moved on in.
        var isEmailStep: Bool {
            if case .email = self { return true }
            return false
        }
    }

    @State var phase: Phase = .email
    @State var email = ""
    @State var password = ""
    @State var approved = false
    /// Whether the person has accepted the certificate the last connect was refused for. Reset
    /// whenever a different certificate arrives, so an acceptance never carries over to one
    /// nobody has looked at.
    @State var certificateAccepted = false
    /// What the found card offers the account for, and what the person has made of it.
    @State var uses = UseChoices(.none)
    /// A use switched on whose server is still to be typed, marked when Connect or the sign-in
    /// was pressed.
    @State var flagged = MissingServer.none
    /// The mandatory Early Access confirmation for a detected Google account; sign-in stays
    /// disabled until it is on (same gate as the manual Google form).
    @State var googleEarlyAccessConfirmed = false
    /// Whether the detected JMAP server advertises OAuth sign-in, as answered by `jmapOAuthProbe`.
    @State var jmapSignInOffered = false
    /// What the detected IMAP server said it accepts. `.checking` until it answers, so the card
    /// draws no credential field in the meantime: one that appears and is then taken away reads
    /// as the app changing its mind (docs/mail-oauth.md rule 8).
    @State var imapAuth: ImapAuthState = .checking
    /// Set while the IMAP sign-in's browser is up.
    @State var imapSigningIn = false
    /// The person asked for the password route beside a sign-in.
    @State var passwordChosen = false

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
                    case let .found(setup): foundView(setup)
                    case .manual: EmptyView() // handled above
                    }
                }
            }
        }
        // An offer only fills the field; it never skips a step. Guarded on the first step and on
        // an empty field, so a re-entrant appear cannot overwrite what somebody has typed.
        .onAppear {
            if phase.isEmailStep, email.isEmpty { email = model.setupStartEmail }
            // An offer opened from elsewhere, the Settings list, lands on its own route, the
            // same as one pressed on this screen.
            if phase.isEmailStep, let offer = model.setupStartOffer { takeOffer(offer) }
        }
        .task { await driveShowcaseIfNeeded() }
        // A different certificate is a different decision, so an acceptance never carries
        // over to one nobody has been shown.
        .onChange(of: model.setupRejectedCertificate) { certificateAccepted = false }
    }

    /// Cancel, where there is somewhere to go: an add over the running app, or back to the link
    /// step an "Add another account" left.
    var cancel: (() -> Void)? {
        model.setupCancellable ? { model.cancelSetup() } : nil
    }

    /// Sets an offered account up on the route its record names, rather than re-deriving one from
    /// the address. That round trip is what syncing an account list exists to save, and for a
    /// domain that publishes no autoconfig it would find *less*, dropping the person onto the
    /// manual form for an account another device set up without trouble.
    ///
    /// The password is still asked for on this device, because no password travels.
    private func takeOffer(_ offer: AllodiaAccountOffer) {
        email = offer.email
        show(offeredSetup(offer: offer))
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
        show(await model.detectSetup(email: email))
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
            OnboardingAllodiaCard(model: model, setUp: takeOffer, firstRun: firstRun)

            TextField(L10n.setup_detect_email_placeholder(), text: $email)
                .setupField(.email)
            SetupFooter {
                if let cancel {
                    Button(L10n.action_cancel(), action: cancel)
                        .keyboardShortcut(.cancelAction)
                }
                Button(L10n.setup_detect_manual()) { phase = .manual(nil, nil) }
                Button(L10n.setup_detect_action()) {
                    Task {
                        phase = .detecting
                        show(await model.detectSetup(email: email))
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
    private func manualView(_ reason: MissReason?, _ edit: SetupRecommendation?) -> some View {
        let prefill = manualPrefill(edit, typedEmail: email)
        AccountSetupView(
            model: model,
            cancel: cancel,
            back: stepBack,
            initialKind: prefill.kind,
            prefillEmail: prefill.email,
            prefillImapHost: prefill.imapHost,
            prefillSmtpHost: prefill.smtpHost,
            prefillJmapServer: prefill.jmapServer,
            note: reason.map(reasonNote)
        )
    }

    /// Back to the address, which keeps what was typed there. Everything the abandoned route
    /// filled in goes with it: a different address can reach a different server, and nothing is
    /// accepted that was not shown.
    func stepBack() {
        model.clearSetupAttempt()
        password = ""
        approved = false
        certificateAccepted = false
        uses = UseChoices(.none)
        flagged = .none
        googleEarlyAccessConfirmed = false
        jmapSignInOffered = false
        passwordChosen = false
        phase = .email
    }

    /// Moves on to what detection found: its card, with the uses as the core said they start, or
    /// the manual form with the reason it ended up there.
    func show(_ setup: DetectedSetup) {
        uses = UseChoices(UseOffer(setup))
        flagged = .none
        if !setup.calendarAndContacts, case let .manual(reason) = setup.recommendation {
            phase = .manual(reason, nil)
        } else {
            phase = .found(setup)
        }
    }
}
