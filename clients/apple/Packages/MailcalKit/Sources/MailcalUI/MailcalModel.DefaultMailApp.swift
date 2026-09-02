// The model's half of "make this your default mail app": asking the core whether to offer, and
// telling it what came of the offer. The platform call is DefaultMailApp.swift and the two
// surfaces are DefaultMailAppViews.swift. Contract: docs/os-integration.md.

import Foundation

extension MailboxModel {

    /// Puts the offer up, if the core says it is due.
    ///
    /// Every condition lives in the core: not before the first account exists, not when the app is
    /// already the default, and not twice. The client contributes only what the core cannot know,
    /// what this *build* can do and whether it is already the handler, so there is no second
    /// answer here that could disagree with what Settings shows.
    func offerDefaultMailAppIfDue() {
        guard let app else { return }
        guard app.shouldOfferDefaultMailApp(
            support: DefaultMailApp.support,
            isDefault: DefaultMailApp.isDefault
        ) else { return }
        offeringDefaultMailApp = true
    }

    /// The user took the offer: ask the OS, and record that the offer is spent.
    ///
    /// Recorded as accepted even though macOS then shows its own consent alert whose answer we
    /// never see. What is being remembered is that *we asked*, not what the system decided: the
    /// alternative re-asks on the next launch every time someone hesitated, which is precisely
    /// the nagging the once-only rule exists to prevent. The Settings row remains for anyone who
    /// changes their mind.
    func acceptDefaultMailApp() {
        DefaultMailApp.requestDefault()
        app?.recordDefaultMailAppOffer(outcome: .accepted)
        offeringDefaultMailApp = false
    }

    /// The user turned the offer down, or closed it without answering. Both end it.
    func declineDefaultMailApp() {
        app?.recordDefaultMailAppOffer(outcome: .declined)
        offeringDefaultMailApp = false
    }

    /// What came of the offer, or `nil` if it has not been put: `true` taken, `false` declined.
    /// The Settings row reads it to say where things stand.
    var defaultMailAppOffer: Bool? { app?.defaultMailAppOffer() }
}
