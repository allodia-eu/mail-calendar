// The core's notices on the model: pulled on `Surface::Notices`, announced once when raised, and
// each action run through the flow this client already has for it (docs/notices.md rule 7).

import MailcalBindings
import SwiftUI

extension MailboxModel {
    /// Reads the notices back, announcing every one that was not in force before. The first pull
    /// announces nothing: what is wrong at launch is on screen, and VoiceOver reads it there.
    func reloadNotices() {
        guard let app else { return }
        let next = app.notices().notices
        if let previous = notices {
            for notice in NoticeRules.raised(next, since: previous) {
                AccessibilityNotification.Announcement(NoticeRules.text(notice.title)).post()
            }
        }
        notices = next
    }

    /// The notices `destination` shows, in the core's order.
    func shownNotices(on destination: AppDestination) -> [Notice] {
        NoticeRules.shown(notices ?? [], on: destination)
    }

    /// Every notice about `account`, in the core's order.
    func notices(about account: String) -> [Notice] {
        NoticeRules.about(account, in: notices ?? [])
    }

    /// Closes a notice about something that already happened; the core ignores any other.
    func dismissNotice(_ notice: Notice) {
        app?.dispatch(intent: .notices(intent: .dismiss(id: notice.id)))
    }

    /// Runs `notice`'s action. `review` is how the view presents the server-change question,
    /// which is a sheet of its own; every other action is run here.
    func runNoticeAction(_ notice: Notice, review: (ServerChangeReview) -> Void) {
        guard let action = notice.action else { return }
        switch action {
        case .openAccountSettings:
            if let account = notice.account { openAccountPage(account) }
        case .review:
            if let account = notice.account, let question = serverChangeReview(account) {
                review(question)
            }
        case .retry:
            retry(notice.screens)
        case .signInToAllodia:
            Task {
                switch await signInToAllodia() {
                case .signedIn: await syncAllodiaAccounts()
                case .cancelled: break
                case .failed: remedyFailed(notice)
                }
            }
        case .signInAgain:
            guard let account = notice.account else { return }
            Task {
                if await !signInAgain(fromNotice: account) { remedyFailed(notice) }
            }
        }
    }

    /// Tells the core `notice`'s remedy did not complete, so it says so beside the notice.
    private func remedyFailed(_ notice: Notice) {
        app?.dispatch(intent: .notices(intent: .remedyFailed(id: notice.id)))
    }

    /// A refresh of what the notice's screen shows, which dials the account again.
    private func retry(_ screens: NoticeScreens) {
        switch screens {
        case .calendar: app?.dispatch(intent: .refreshCalendar)
        case .contacts: app?.dispatch(intent: .refreshContacts)
        case .mail, .everywhere: refresh()
        }
    }

    /// Signs `account` in again the way its sign-in was made, answering whether that either
    /// worked or was left by the person; only a failure is said on the notice. A password or
    /// pasted-secret account has no sign-in to run here, so its page opens instead.
    private func signInAgain(fromNotice account: String) async -> Bool {
        let provider = app?.accountProvider(accountId: account)
        let nextcloud = accountEntry(account)?.nextcloud == true
        switch AccountSignInRules.remedy(provider, nextcloud: nextcloud) {
        case .consent:
            return await signInAgain(account).completedOrLeft
        case .accountSignIn:
            return await signInThroughProvider(account, redirectURI: ImapOAuthConfig.redirectURI)
                .completedOrLeft
        case .jmapReauth:
            return await reconnectJmap(accountId: account)
        case .nextcloud:
            // Reports a failure through its own callback, after the browser round trip.
            signInAccountWithNextcloud(account) { [weak self] _ in
                guard let self, let notice = self.notices(about: account).first(where: {
                    $0.action == .signInAgain
                }) else { return }
                self.remedyFailed(notice)
            }
            return true
        case .settings:
            openAccountPage(account)
            return true
        }
    }
}

extension ConsentEnded {
    /// The sign-in worked, or the person closed it: neither is a failure to report.
    var completedOrLeft: Bool {
        if case .failed = self { return false }
        return true
    }
}
