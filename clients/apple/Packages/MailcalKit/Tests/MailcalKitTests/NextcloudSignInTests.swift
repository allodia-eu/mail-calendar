// Signing in with Nextcloud (docs/nextcloud-sign-in.md): the poll and what ends it (rule 5), the
// grant kept for another try and withdrawn when nobody will use it (rule 6), the manual form
// asking once per server, and the account page's section and the banner's remedy (rules 2 and 7).
// That the browser opens is hand-verified, because reaching it opens a real one.

import MailcalBindings
import Testing

@testable import MailcalUI

private let grant = NextcloudGrant(value: "{\"secret\":true}")

/// Answers each poll from a script, recording how many were asked.
@MainActor
private final class Script {
    var polls: [Result<NextcloudSignInPoll, Error>]
    var pauses: [NextcloudWakeSignal]
    private(set) var asked = 0

    init(polls: [Result<NextcloudSignInPoll, Error>], pauses: [NextcloudWakeSignal]) {
        self.polls = polls
        self.pauses = pauses
    }

    func poll() -> Result<NextcloudSignInPoll, Error> {
        asked += 1
        return polls.removeFirst()
    }

    func pause() -> NextcloudWakeSignal { pauses.removeFirst() }
}

@MainActor
private func flight(_ script: Script) async -> NextcloudFlightEnd {
    await awaitNextcloudGrant(poll: { script.poll() }, pause: { script.pause() })
}

@MainActor
@Suite struct NextcloudFlightTests {
    @Test func aGrantEndsThePollAfterWaiting() async {
        let script = Script(
            polls: [.success(.waiting), .success(.waiting), .success(.granted(grant: grant.value))],
            pauses: [.tick, .returned])
        #expect(await flight(script) == .granted(grant))
        #expect(script.asked == 3)
    }

    @Test func anExpiredSignInSaysItTimedOut() async {
        let script = Script(polls: [.success(.expired)], pauses: [])
        let end = await flight(script)
        #expect(end == .expired)
        #expect(end.said == L10n.account_signin_timeout())
    }

    @Test func aClosedBrowserGetsOneMorePollThenRetires() async {
        let waiting = Script(
            polls: [.success(.waiting), .success(.waiting)], pauses: [.closed])
        #expect(await flight(waiting) == .cancelled)
        #expect(waiting.asked == 2)
        // Granted the moment before the sheet closed: that last poll finds it.
        let granted = Script(
            polls: [.success(.waiting), .success(.granted(grant: grant.value))], pauses: [.closed])
        #expect(await flight(granted) == .granted(grant))
    }

    @Test func aBrowserThatFailedStopsThePoll() async {
        let script = Script(polls: [.success(.waiting)], pauses: [.browserFailed])
        #expect(await flight(script) == .failed(L10n.account_signin_browser_failed()))
        #expect(script.asked == 1)
    }

    @Test func aPollTheServerRefusesIsTheSignInFailing() async {
        let script = Script(
            polls: [.failure(MailcalError.Connect("unreadable"))], pauses: [])
        #expect(await flight(script) == .failed(L10n.setup_imap_signin_failed()))
    }

    @Test func aRetiredAttemptEndsCancelledAndSaysNothing() async {
        let script = Script(polls: [.success(.waiting)], pauses: [.cancelled])
        let end = await flight(script)
        #expect(end == .cancelled)
        #expect(end.said == nil)
    }

    @Test func theGrantIsNeverPrinted() {
        #expect(!"\(grant)".contains("secret"))
        #expect(!String(reflecting: grant).contains("secret"))
    }

    @Test func aWakeEndsThePauseAndAClosedBrowserWhilePollingIsKept() async {
        let wake = NextcloudWake()
        // Closed while a poll ran: the next pause ends at once rather than waiting the interval.
        wake.fire(.closed)
        #expect(await wake.pause(for: .seconds(60)) == .closed)
        // A return while polling is not kept, since the poll it asks for is running already.
        wake.fire(.returned)
        Task { @MainActor in wake.fire(.returned) }
        #expect(await wake.pause(for: .seconds(60)) == .returned)
        #expect(await wake.pause(for: .milliseconds(10)) == .tick)
    }
}

@Suite struct NextcloudSignInStateTests {
    @Test func aGrantForTheAttemptInFlightIsKeptWhileItIsCompleted() {
        var state = NextcloudSignInState()
        let (attempt, withdraw) = state.begin()
        #expect(withdraw == nil)
        #expect(state.waiting)
        #expect(state.landed(attempt, key: "server", .granted(grant)) == .complete(grant))
        #expect(state.completing)
        #expect(state.completed(.done) == nil)
        #expect(state.held == nil)
    }

    @Test func aFailedCompletionKeepsTheGrantForTheNextPressWithoutTheBrowser() {
        var state = NextcloudSignInState()
        let (attempt, _) = state.begin()
        _ = state.landed(attempt, key: "server", .granted(grant))
        #expect(state.completed(.failed(said: "refused")) == nil)
        #expect(state.held == .init(key: "server", grant: grant))
        #expect(state.retry("another server") == nil)
        #expect(state.retry("server") == grant)
        #expect(state.completing)
    }

    @Test func aGrantKeptForOneServerIsWithdrawnWhenAnotherSignInStarts() {
        var state = NextcloudSignInState()
        let (attempt, _) = state.begin()
        _ = state.landed(attempt, key: "server", .granted(grant))
        _ = state.completed(.failed(said: nil))
        #expect(state.begin().withdraw == grant)
        #expect(state.held == nil)
    }

    @Test func aGrantForASupersededOrRetiredAttemptIsWithdrawn() {
        var state = NextcloudSignInState()
        let (first, _) = state.begin()
        let (second, _) = state.begin()
        #expect(state.landed(first, key: "server", .granted(grant)) == .withdraw(grant))
        state.retire()
        #expect(state.landed(second, key: "server", .granted(grant)) == .withdraw(grant))
        #expect(state.landed(second, key: "server", .expired) == .dropped)
        #expect(state.held == nil)
    }

    @Test func leavingWithdrawsAKeptGrant() {
        var state = NextcloudSignInState()
        let (attempt, _) = state.begin()
        _ = state.landed(attempt, key: "server", .granted(grant))
        _ = state.completed(.failed(said: nil))
        #expect(state.leave() == grant)
        #expect(state.leave() == nil)
    }

    @Test func leavingNeverWithdrawsAGrantTheCoreIsCompleting() {
        var succeeded = NextcloudSignInState()
        var (attempt, _) = succeeded.begin()
        _ = succeeded.landed(attempt, key: "server", .granted(grant))
        #expect(succeeded.leave() == nil)
        #expect(succeeded.completed(.done) == nil)

        // The same, but the completion failed after the surface was left: nobody will retry it.
        var failed = NextcloudSignInState()
        (attempt, _) = failed.begin()
        _ = failed.landed(attempt, key: "server", .granted(grant))
        #expect(failed.leave() == nil)
        #expect(failed.completed(.failed(said: nil)) == grant)
        #expect(failed.held == nil)
    }
}

@Suite struct NextcloudManualAskTests {
    @Test func aServerIsAskedAboutOnceAndOfferedWhileTheFormStillNamesIt() {
        var ask = NextcloudManualAsk()
        #expect(ask.question("", "") == nil)
        #expect(ask.question(" https://cloud.example/ ", "") == "https://cloud.example/")
        // Leaving the field again asks nothing new.
        #expect(ask.question("https://cloud.example/", "") == nil)
        ask.answered("https://cloud.example/", offers: true)
        #expect(ask.offer("https://cloud.example/", "") == "https://cloud.example/")
        // A different server typed since: the yes was for the old one.
        #expect(ask.offer("https://other.example/", "") == nil)
    }

    @Test func theAddressBookIsAskedWhenNoCalendarIsNamedAndANoOffersNothing() {
        var ask = NextcloudManualAsk()
        #expect(ask.question("", "https://book.example/") == "https://book.example/")
        ask.answered("https://book.example/", offers: false)
        #expect(ask.offer("", "https://book.example/") == nil)
        // An answer for a server no longer asked about is dropped.
        _ = ask.question("https://cloud.example/", "")
        ask.answered("https://book.example/", offers: true)
        #expect(ask.offer("", "https://book.example/") == nil)
    }
}

@Suite struct NextcloudAccountRulesTests {
    private func options(nextcloud: Bool) -> AccountSignInOptions {
        AccountSignInOptions(signIn: false, providerLabel: nil, password: false, nextcloud: nextcloud)
    }

    @Test func anAccountANextcloudSignInMadeSignsInAgainAndNeverSwitchesToAPassword() {
        let asking = NextcloudAccountRules.section(nextcloud: true, nil)
        #expect(asking?.status == L10n.settings_account_signs_in_nextcloud())
        #expect(asking?.action == nil)
        let section = NextcloudAccountRules.section(nextcloud: true, options(nextcloud: true))
        #expect(section?.action == .signInAgain)
    }

    @Test func aTypedPasswordAtANextcloudServerOffersTheSwitch() {
        let section = NextcloudAccountRules.section(nextcloud: false, options(nextcloud: true))
        #expect(section?.status == L10n.settings_account_signs_in_password())
        #expect(section?.action == .toProvider(label: "Nextcloud"))
        #expect(
            AccountSettingsLabels.signInAction(section!.action!)
                == L10n.settings_account_switch_to_provider_named(provider: "Nextcloud"))
    }

    @Test func anyOtherCalendarAccountHasNoSection() {
        #expect(NextcloudAccountRules.section(nextcloud: false, nil) == nil)
        #expect(NextcloudAccountRules.section(nextcloud: false, options(nextcloud: false)) == nil)
    }

    @Test func theBannerSignsANextcloudAccountInWithNextcloudWhateverItsProvider() {
        #expect(AccountSignInRules.remedy(.password, nextcloud: true) == .nextcloud)
        #expect(AccountSignInRules.remedy(nil, nextcloud: true) == .nextcloud)
        #expect(AccountSignInRules.remedy(.password) == .settings)
    }

    @Test func aRefusedNextcloudAddOffersTheAccountAlreadySetUp() {
        let offer = ExistingAccountUpdate(nextcloudRefusal: MailcalError.AlreadySetUp(accountId: "a"))
        #expect(offer?.accountId == "a")
        #expect(offer?.nextcloud == true)
        #expect(ExistingAccountUpdate(nextcloudRefusal: MailcalError.DifferentAccount) == nil)
    }
}
