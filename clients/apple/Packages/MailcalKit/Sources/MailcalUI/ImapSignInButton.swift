// What a mail account's setup screen asks for, once its server has answered: the state, the line
// that explains it, and the sign-in button.
//
// Three states rather than a flag, from docs/mail-oauth.md rule 2, and the middle one is why: a
// provider whose sign-in exists but admits only applications it registered in advance is not the
// same as one that offers none, and showing one bare password form for both leaves somebody
// wondering why the button their colleague has is missing.
//
// The password route is never removed where it works. Sign-in leads, because that is what the
// server said it prefers; a password field sits below it and always connects.

import MailcalBindings // L10n
import SwiftUI

/// What the mail server said it accepts, plus the two states the *card* has that the server does
/// not: still asking, and a sign-in that was started and did not finish.
enum ImapAuthState: Equatable {
    /// The server has not answered. Nothing to act on is drawn: a credential field that appears
    /// and is then taken away reads as the app changing its mind.
    case checking
    /// Sign in with the provider. `passwordAlsoWorks` decides whether the password field sits
    /// below it, or whether that route would be a dead end.
    case signIn(passwordAlsoWorks: Bool)
    /// The provider's sign-in exists but is closed to this application.
    case registrationNeeded
    /// No sign-in here: the password form, as it always was.
    case password
    /// A sign-in was started and did not finish. The password field comes back, because that is
    /// the route left, and the reason is said rather than left to be guessed at.
    case failed

    /// How long the card waits before falling back to the password field.
    ///
    /// The card shows nothing to act on while it asks, so a server that never answers must not be
    /// able to hold somebody there. Long enough for a TLS handshake to a slow host plus a couple
    /// of metadata requests.
    static let deadline = Duration.seconds(10)

    init(_ offer: ImapAuthOffer) {
        switch offer {
        case let .signIn(_, _, passwordAlsoWorks):
            self = .signIn(passwordAlsoWorks: passwordAlsoWorks)
        case .registrationNeeded:
            self = .registrationNeeded
        case .password:
            self = .password
        }
    }

    /// Whether the sign-in button belongs on screen.
    var offersSignIn: Bool {
        if case .signIn = self { return true }
        return false
    }

    /// Whether to explain that this provider admits only pre-registered applications.
    var explainsRegistration: Bool {
        if case .registrationNeeded = self { return true }
        return false
    }

    /// Whether the password field belongs on screen.
    ///
    /// Not while the server is still being asked, and not when it said a password is refused: on
    /// a provider that has switched password authentication off, that field is a dead end nobody
    /// finds until they have typed one.
    var showsPassword: Bool {
        switch self {
        case .checking: return false
        case let .signIn(passwordAlsoWorks): return passwordAlsoWorks
        case .registrationNeeded, .password, .failed: return true
        }
    }
}

/// The account a pre-flight and a sign-in both describe.
///
/// One builder, used by both, so the two cannot come to different conclusions about the same
/// account: a pre-flight that probed a different server from the one the sign-in registers
/// against would offer a button that fails at the provider.
func imapLoginRequest(
    email: String,
    imapHost: String,
    smtpHost: String?,
    caldavURL: String?,
    imapSecurity: ConnectionSecurity,
    smtpSecurity: ConnectionSecurity,
    oauthIssuer: String?
) -> ImapLoginRequest {
    ImapLoginRequest(
        email: email,
        imapHost: imapHost,
        smtpHost: smtpHost,
        caldavBaseUrl: caldavURL,
        imapSecurity: imapSecurity,
        smtpSecurity: smtpSecurity,
        oauthIssuer: oauthIssuer
    )
}

/// The line that says what the server answered, when it says something.
///
/// Silent in the ordinary case, a provider that takes a password and always did: there is nothing
/// to explain, and a line saying so would be noise on every setup.
struct ImapAuthExplanation: View {
    let state: ImapAuthState

    var body: some View {
        switch state {
        case .checking:
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(L10n.setup_imap_signin_checking()).foregroundStyle(.secondary)
            }
            .font(.caption)
        case .signIn:
            caption(L10n.setup_imap_signin_note(), .secondary)
        case .registrationNeeded:
            caption(L10n.setup_imap_signin_registration_needed(), .secondary)
        case .failed:
            caption(L10n.setup_imap_signin_failed(), .red)
        case .password:
            EmptyView()
        }
    }

    private func caption(_ text: String, _ colour: Color) -> some View {
        Text(text)
            .font(.caption).foregroundStyle(colour)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// The sign-in button, shown only where the server said it takes one.
struct ImapSignInButton: View {
    /// The account the sign-in is for: the same request the pre-flight asked about.
    let request: ImapLoginRequest
    /// Runs the browser sign-in and, on success, adds + stores the account.
    let signIn: (ImapLoginRequest) async -> ImapSignInOutcome
    /// Told when a sign-in was started and did not finish, so the card can bring the password
    /// route back and say why.
    let failed: () -> Void

    @State private var signingIn = false

    var body: some View {
        if signingIn {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text(L10n.status_connecting()).foregroundStyle(.secondary)
            }
        } else {
            Button(L10n.setup_imap_signin_button()) { start() }
        }
    }

    private func start() {
        Task {
            signingIn = true
            let outcome = await signIn(request)
            signingIn = false
            // A dismissed browser is not a failure: say nothing and leave the button as it was.
            if case .failed = outcome { failed() }
        }
    }
}
