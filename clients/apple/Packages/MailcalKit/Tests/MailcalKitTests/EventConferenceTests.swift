// What the detail says about a video meeting, and what its links open, tested without a view
// (docs/conferencing.md). The links are the part that matters most: Join and a dial-in number are
// the two places a tap leaves the app, so what each will open is pinned here.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct EventConferenceTests {
    private func meeting(
        service: String? = "Microsoft Teams",
        joinUrl: String? = "https://teams.microsoft.com/meet/123?p=x",
        state: ConferenceState = .ready
    ) -> EventConference {
        EventConference(
            service: service,
            joinUrl: joinUrl,
            meetingId: "123 456",
            passcode: nil,
            dialIn: [],
            notes: nil,
            state: state
        )
    }

    @Test func aMeetingIsHeadedByItsServiceAndOtherwiseByTheCatalogsWords() {
        #expect(conferenceTitle(meeting()) == "Microsoft Teams")
        #expect(conferenceTitle(meeting(service: nil)) == L10n.event_video_meeting())
    }

    @Test func joinOpensTheCoresWebLinkAndNothingElse() {
        #expect(conferenceJoinURL(meeting())?.absoluteString == "https://teams.microsoft.com/meet/123?p=x")
        #expect(conferenceJoinURL(meeting(joinUrl: nil)) == nil)
        #expect(conferenceJoinURL(meeting(joinUrl: "javascript:alert(1)")) == nil)
    }

    @Test func aMeetingStillBeingMadeSaysSoInsteadOfOfferingJoin() {
        #expect(conferenceStatus(meeting()) == nil)
        #expect(conferenceStatus(meeting(joinUrl: nil, state: .preparing)) == L10n.event_meeting_preparing())
        #expect(conferenceStatus(meeting(joinUrl: nil, state: .failed)) == L10n.event_meeting_failed())
    }

    @Test func aDialInRowOpensOnlyATelephoneNumber() {
        let number = ConferenceDialIn(number: "+31 20 555 0100", uri: "tel:+31205550100", pin: "990#")
        #expect(dialInURL(number)?.absoluteString == "tel:+31205550100")
        let web = ConferenceDialIn(number: "x", uri: "https://example.com/", pin: nil)
        #expect(dialInURL(web) == nil)
    }

    @Test func anEventWithAMeetingSaysSoToVoiceOver() {
        let plain = calendarEventLabel(title: "Sync", time: "09:00", calendar: "Work", participation: .accepted)
        let withMeeting = calendarEventLabel(
            title: "Sync", time: "09:00", calendar: "Work", participation: .accepted, hasConference: true
        )
        #expect(withMeeting == "\(plain), \(L10n.event_video_meeting())")
    }
}
