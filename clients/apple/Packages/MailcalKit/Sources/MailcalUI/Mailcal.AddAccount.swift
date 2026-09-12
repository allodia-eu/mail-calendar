// The add-another-account sheet: the same setup form as first-run, over the running app.
//
// Its own file rather than a member of the shell, for the reason the doc comment below gives:
// SwiftUI's type checker budgets per expression, and this call is large enough to exhaust one
// when it sits in a modifier chain. Split from Mailcal.swift, which is at the line limit.

import SwiftUI

extension ContentView {
    /// The add-another-account form, out of the modifier chain that presents it.
    ///
    /// Not a style choice: SwiftUI's type checker budgets per expression, and a chain of a dozen
    /// modifiers carrying a call this size exhausts it, reporting the failure against whichever
    /// unrelated line it gave up on.
    @ViewBuilder
    var addAccountSheet: some View {
        AccountSetupDetectView(
            error: model.setupError,
            cancel: {
                model.addingAccount = false
                model.setupError = nil
                model.setupStartEmail = ""
                model.setupStartOffer = nil
            },
            signInMicrosoft: { hint in model.signInWithMicrosoft(loginHint: hint) },
            signInGoogle: { hint in model.signInWithGoogle(loginHint: hint) },
            signingIn: model.microsoftSigningIn,
            googleSigningIn: model.googleSigningIn,
            connecting: model.isConnecting,
            submit: { imapHost, username, password, smtpHost, caldavURL, imapSecurity, smtpSecurity in
                model.submitSetup(
                    imapHost: imapHost,
                    username: username,
                    password: password,
                    smtpHost: smtpHost,
                    caldavBaseUrl: caldavURL,
                    imapSecurity: imapSecurity,
                    smtpSecurity: smtpSecurity
                )
            },
            submitJmap: { email, serverURL, password in
                model.submitJmapSetup(
                    email: email,
                    serverURL: serverURL,
                    password: password
                )
            },
            jmapOAuthAvailable: { email, serverURL in
                await model.jmapOAuthAvailable(email: email, serverURL: serverURL)
            },
            signInJmap: { email, serverURL in
                await model.signInWithJmap(email: email, serverURL: serverURL)
            },
            detect: { email in await model.detectSetup(email: email) },
            startEmail: model.setupStartEmail,
            startOffer: model.setupStartOffer,
            // Not the first account, so no card, but the accounts still to set up are not a
            // pitch, and are offered here too (`docs/onboarding.md`).
            onboarding: model,
            firstRun: false
        )
    }
}
