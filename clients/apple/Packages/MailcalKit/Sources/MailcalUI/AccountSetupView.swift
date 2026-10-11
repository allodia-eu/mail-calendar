// The manual account-setup form: pick an account type, and that type's pane fills the rest
// (AccountSetupManualForms.swift). Reached from "Set up manually" on the address step, from the
// same button on a found card (prefilled with what detection found), and whenever detection comes
// back empty. On submit the model serializes the fields, connects, and the core stores them.

import SwiftUI
import MailcalBindings

/// The account kinds the setup form can offer.
enum SetupAccountKind: Hashable {
    case imap
    case jmap
    /// A calendar and address book without mail (CalDAV and CardDAV).
    case dav
    case microsoft
    case google

    /// The kinds this build offers, in picker order. A browser sign-in needs the provider's
    /// OAuth client registration, which is injected at build time, so a build given none drops
    /// the route rather than showing a button that fails at the provider. The credential routes
    /// are always there.
    static var offered: [SetupAccountKind] {
        let routes = oauthRoutes()
        return [.imap, .jmap, .dav, .microsoft, .google].filter { kind in
            switch kind {
            case .microsoft: return routes.microsoft
            case .google: return routes.google
            case .imap, .jmap, .dav: return true
            }
        }
    }

    var label: String {
        switch self {
        case .imap: return L10n.setup_account_type_password()
        case .jmap: return L10n.setup_account_type_jmap()
        case .dav: return L10n.setup_account_type_dav()
        case .microsoft: return L10n.setup_account_type_microsoft()
        case .google: return L10n.setup_account_type_google()
        }
    }

    /// The provider whose sign-in this kind runs, for the uses it offers before the browser opens.
    var provider: AccountKind? {
        switch self {
        case .microsoft: .microsoft
        case .google: .google
        case .imap, .jmap, .dav: nil
        }
    }
}

struct AccountSetupView: View {
    var model: MailboxModel
    /// Shown when there is somewhere to cancel to; `nil` on the first run.
    var cancel: (() -> Void)?
    /// Back to the address step, when the email-first flow routed here.
    var back: (() -> Void)?
    /// Shown when the email-first detection flow routed here (nothing found, offline, an
    /// unsupported provider), so the user knows why they're entering settings by hand.
    var note: String?

    @State var kind: SetupAccountKind
    /// What the typed mail server said it accepts. `.password` until it answers, because this
    /// pane's password field is already on screen and must not be taken away to say nothing.
    @State var imapAuth: ImapAuthState = .password
    /// Set while the IMAP sign-in's browser is up.
    @State var imapSigningIn = false
    /// The person asked for the password route beside a sign-in.
    @State var passwordChosen = false

    @State var imapHost: String
    @State var username: String
    @State var password = ""
    @State var smtpHost: String
    @State var caldavURL = ""
    @State var carddavURL = ""
    /// Each server's port and connection security. The host fields hold the name alone; the port
    /// sits beside it, where the user can see and change it.
    @State var servers = ManualServerPair()
    @State var jmapServer: String
    /// What a Microsoft or Google sign-in is asked for, as the core offers it for the address
    /// typed: a personal address is offered no colleagues.
    @State var providerUses = UseChoices(.none)
    /// The calendar-and-contacts form's server fields, marked when Connect found neither filled.
    @State var flagged = MissingServer.none
    /// Focus moves to the calendar server when Connect found neither server filled, which is what
    /// a screen reader announces of the marking.
    @FocusState var calendarServerFocused: Bool
    /// The mandatory Early Access confirmation for Google; the "Sign in with Google" button stays
    /// disabled until it is on (Gmail is allow-listed while Google verifies the app).
    @State var googleEarlyAccessConfirmed = false
    /// Whether the typed JMAP server advertises OAuth sign-in, as answered by `jmapOAuthProbe`.
    /// False until the core says otherwise, so the button is never offered on a guess.
    @State var jmapSignInOffered = false
    /// Whether the person has accepted the certificate the last connect was refused for. Reset
    /// whenever a different one arrives, so an acceptance never carries over.
    @State var certificateAccepted = false

    /// The manual form, optionally prefilled from a detection result the user chose to edit
    /// (and with a `note` explaining why detection routed here).
    init(
        model: MailboxModel,
        cancel: (() -> Void)? = nil,
        back: (() -> Void)? = nil,
        initialKind: SetupAccountKind = .imap,
        prefillEmail: String = "",
        prefillImapHost: String = "",
        prefillSmtpHost: String = "",
        prefillJmapServer: String = "",
        note: String? = nil
    ) {
        self.model = model
        self.cancel = cancel
        self.back = back
        self.note = note
        _kind = State(initialValue: initialKind)
        _imapHost = State(initialValue: prefillImapHost)
        _username = State(initialValue: prefillEmail)
        _smtpHost = State(initialValue: prefillSmtpHost)
        _jmapServer = State(initialValue: prefillJmapServer)
    }

    /// The refused certificate this pane can actually do something about. Only a standards
    /// account's stored config carries an exception, so a JMAP refusal is reported and not
    /// offered: taking an answer and ignoring it is worse than not asking
    /// (`docs/certificate-exceptions.md` → Known gaps).
    var refusedCertificate: RejectedCertificate? {
        kind == .imap || kind == .dav ? model.setupRejectedCertificate : nil
    }

    /// Connect stays inert while a refused certificate is waiting to be accepted, the same
    /// gate the detected card applies.
    var certificateOK: Bool { refusedCertificate == nil || certificateAccepted }

    /// The certificate to store with the account: the refused one, once accepted.
    var acceptedCertificate: RejectedCertificate? {
        certificateAccepted ? refusedCertificate : nil
    }

    var body: some View {
        SetupScaffold {
            Text(L10n.setup_title()).font(.title2).bold()

            if let note {
                Text(note).font(.callout).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            // A menu rather than segments: five kinds, one of them named at length, do not fit
            // a phone's width side by side.
            HStack {
                Text(L10n.setup_account_type())
                Spacer()
                Picker(L10n.setup_account_type(), selection: $kind) {
                    ForEach(SetupAccountKind.offered, id: \.self) { offered in
                        Text(offered.label).tag(offered)
                    }
                }
                .pickerStyle(.menu)
                .labelsHidden()
                .fixedSize()
            }

            switch kind {
            case .imap: imapForm
            case .jmap: jmapForm
            case .dav: davForm
            case .microsoft: microsoftForm
            case .google: googleForm
            }

            // The transport's own words, suppressed while the certificate panel below is up:
            // that panel says the same thing in the reader's language and with the certificate
            // beside it, and the raw text above it is a second, worse copy of the question.
            if let error = model.setupError, refusedCertificate == nil {
                Text(error).font(.callout).foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }
            if let refusedCertificate {
                CertificateExceptionPanel(
                    certificate: refusedCertificate,
                    accepted: $certificateAccepted
                )
            }

            SetupFooter(back: back, backDisabled: busy) {
                if let cancel {
                    Button(L10n.action_cancel(), action: cancel)
                        .keyboardShortcut(.cancelAction)
                }
                primaryAction
            }
        }
        // A different certificate is a different decision, so an acceptance never carries
        // over to one nobody has been shown.
        .onChange(of: model.setupRejectedCertificate) { certificateAccepted = false }
        // The uses a provider offers follow the address typed; drawn again only when they change,
        // so a choice already made is not reset on every keystroke.
        .task(id: ProviderOfferKey(kind: kind.provider, choices: providerOffer.choices)) {
            providerUses = UseChoices(providerOffer)
        }
    }

    /// A connect or a sign-in is running, and its answer belongs to the step on screen.
    var busy: Bool {
        model.isConnecting || model.microsoftSigningIn || model.googleSigningIn || imapSigningIn
    }

    /// What a Microsoft or Google sign-in for the typed address is offered; nothing for any other
    /// kind, whose servers decide.
    var providerOffer: UseOffer {
        kind.provider.map { UseOffer.provider($0, email: username.trimmingCharacters(in: .whitespaces)) }
            ?? .none
    }

    /// The footer's leading action, which is whichever verb the selected account kind needs:
    /// Connect for the credential routes, Sign in for the two OAuth ones, or the progress readout
    /// that replaces it while one is running.
    @ViewBuilder private var primaryAction: some View {
        switch kind {
        case .imap:
            if model.isConnecting {
                progress(L10n.status_connecting())
            } else if !imapAuth.showsPasswordField(chosen: passwordRouteChosen) {
                // The sign-in leads from the card, under the line that explains it; or the server
                // takes neither a password nor this application, and there is nothing to connect.
                EmptyView()
            } else {
                Button(L10n.action_connect(), action: submitImap)
                    .buttonStyle(.borderedProminent)
                    .disabled(!canConnect || !certificateOK)
            }
        case .jmap:
            if model.isConnecting {
                progress(L10n.status_connecting())
            } else {
                Button(L10n.action_connect()) {
                    model.submitJmapSetup(email: username, serverURL: jmapServer, password: password)
                }
                .buttonStyle(.borderedProminent)
                .disabled(!canConnectJmap)
            }
        case .dav:
            if model.isConnecting {
                progress(L10n.status_connecting())
            } else {
                Button(L10n.action_connect(), action: submitDav)
                    .buttonStyle(.borderedProminent)
                    .disabled(!canConnectDav || !certificateOK)
            }
        case .microsoft:
            if model.microsoftSigningIn {
                progress(L10n.setup_microsoft_signing_in())
            } else {
                Button(L10n.setup_microsoft_signin()) {
                    model.signInWithMicrosoft(loginHint: typedHint, uses: providerChoice)
                }
                .buttonStyle(.borderedProminent)
            }
        case .google:
            if model.googleSigningIn {
                progress(L10n.setup_google_signing_in())
            } else {
                // Disabled until the Early Access confirmation is on (see googleForm).
                Button(L10n.setup_google_signin()) {
                    model.signInWithGoogle(loginHint: typedHint, uses: providerChoice)
                }
                .buttonStyle(.borderedProminent)
                .disabled(!googleEarlyAccessConfirmed)
            }
        }
    }
}

/// What the provider's uses are drawn for: the kind, and the uses its sign-in offers the address.
struct ProviderOfferKey: Hashable {
    let kind: AccountKind?
    let choices: [SetupChoice]
}
