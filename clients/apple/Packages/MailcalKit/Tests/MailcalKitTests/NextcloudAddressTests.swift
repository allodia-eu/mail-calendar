// The manual form's Nextcloud pane (docs/nextcloud-sign-in.md rule 2): what each answer about the
// address typed comes to, the offer to sign in anyway while the field holds the address asked,
// and the check's wait giving way to the browser's.

import MailcalBindings
import Testing

@testable import MailcalUI

private let start = NextcloudSignInStart(loginUrl: "https://cloud.test.local/login", pending: "{}")

/// What starting at an address answered with `answer` ends as: `nil` when a sign-in started.
private func refusal(_ answer: NextcloudAddressStart) -> NextcloudFlightEnd? {
    do {
        _ = try nextcloudAddressStarted(answer)
        return nil
    } catch let refusal as NextcloudAddressRefusal {
        return refusal.end
    } catch {
        Issue.record("an unexpected error: \(error)")
        return nil
    }
}

@Suite struct NextcloudAddressAnswerTests {
    @Test func aStartedSignInGoesOnToTheBrowser() throws {
        let started = try nextcloudAddressStarted(.started(start: start))
        #expect(started.loginUrl == start.loginUrl)
    }

    @Test func anUnsureAddressEndsSayingNothingOfItsOwn() {
        let end = refusal(.unsure(host: "cloud.test.local"))
        #expect(end == .unsure(host: "cloud.test.local"))
        #expect(end?.said == nil)
    }

    @Test func eachRefusalSaysWhy() {
        #expect(refusal(.notNextcloud(host: "mail.test.local"))?.said
            == L10n.setup_nextcloud_not_found(host: "mail.test.local"))
        #expect(refusal(.maintenance(host: "cloud.test.local"))?.said
            == L10n.setup_nextcloud_maintenance(host: "cloud.test.local"))
        #expect(refusal(.notAnAddress)?.said == L10n.setup_nextcloud_address_invalid())
    }
}

@Suite struct NextcloudAddressStateTests {
    @Test func anUnsureEndOffersTheSignInAnywayWhileTheFieldHoldsTheAddressAsked() {
        var state = NextcloudSignInState()
        let (attempt, _) = state.begin(checking: true)
        _ = state.landed(attempt, key: "cloud.test.local/apps", .unsure(host: "cloud.test.local"))
        let unsure = state.unsure
        #expect(unsure?.offer("cloud.test.local/apps") == "cloud.test.local")
        #expect(unsure?.offer("  cloud.test.local/apps ") == "cloud.test.local")
        #expect(unsure?.offer("cloud.test.local/app") == nil)
        #expect(unsure?.offer("") == nil)
    }

    @Test func startingASignInClearsTheUnsureNote() {
        var state = NextcloudSignInState()
        let (first, _) = state.begin(checking: true)
        _ = state.landed(first, key: "cloud.test.local", .unsure(host: "cloud.test.local"))
        _ = state.begin()
        #expect(state.unsure == nil)
    }

    @Test func leavingSetupClearsTheUnsureNote() {
        var state = NextcloudSignInState()
        let (attempt, _) = state.begin(checking: true)
        _ = state.landed(attempt, key: "cloud.test.local", .unsure(host: "cloud.test.local"))
        _ = state.leave()
        #expect(state.unsure == nil)
    }

    @Test func aFailedCheckKeepsNoOfferAndSaysWhy() {
        var state = NextcloudSignInState()
        let (attempt, _) = state.begin(checking: true)
        let said = L10n.setup_nextcloud_not_found(host: "mail.test.local")
        #expect(state.landed(attempt, key: "mail.test.local", .failed(said)) == .ended(.failed(said)))
        #expect(state.unsure == nil)
        #expect(!state.waiting)
    }

    @Test func theCheckGivesWayToTheBrowserOnceTheServerStartedTheSignIn() {
        var state = NextcloudSignInState()
        let (attempt, _) = state.begin(checking: true)
        #expect(state.checking)
        #expect(state.waiting)
        state.opened(attempt)
        #expect(!state.checking)
        #expect(state.waiting)
    }

    @Test func aSupersededCheckDoesNotEndTheNextOnesWait() {
        var state = NextcloudSignInState()
        let (first, _) = state.begin(checking: true)
        _ = state.begin(checking: true)
        state.opened(first)
        #expect(state.checking)
    }

    @Test func aSignInAtAServerKnownToBeNextcloudChecksNothing() {
        var state = NextcloudSignInState()
        _ = state.begin()
        #expect(!state.checking)
        #expect(state.waiting)
    }

    @Test func aCancelledCheckWaitsNoLonger() {
        var state = NextcloudSignInState()
        _ = state.begin(checking: true)
        state.retire()
        #expect(!state.checking)
        #expect(!state.waiting)
    }
}

@Suite struct ManualSetupKindTests {
    @Test func thePickerOffersTheCoresKindsInTheCoresOrder() {
        let core = manualSetupKinds()
        #expect(SetupAccountKind.offered == core.map(SetupAccountKind.init))
        #expect(SetupAccountKind.offered.contains(.nextcloud))
    }
}
