// What the detail says about a video meeting, and that its links are the core's as sent, tested
// without a view (docs/conferencing.md). Which links those are is decided and pinned in the core
// (event_conference_tests.rs).

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

    @Test func joinOpensTheCoresLinkAsSent() {
        #expect(conferenceJoinURL(meeting())?.absoluteString == "https://teams.microsoft.com/meet/123?p=x")
        #expect(conferenceJoinURL(meeting(joinUrl: nil)) == nil)
    }

    @Test func aMeetingStillBeingMadeSaysSoInsteadOfOfferingJoin() {
        #expect(conferenceStatus(meeting()) == nil)
        #expect(conferenceStatus(meeting(joinUrl: nil, state: .preparing)) == L10n.event_meeting_preparing())
        #expect(conferenceStatus(meeting(joinUrl: nil, state: .failed)) == L10n.event_meeting_failed())
    }

    @Test func aDialInRowOpensTheCoresNumberAndARefusedOneIsText() {
        let number = ConferenceDialIn(number: "+31 20 555 0100", uri: "tel:+31205550100", pin: "990#")
        #expect(dialInURL(number)?.absoluteString == "tel:+31205550100")
        let refused = ConferenceDialIn(number: "*21*0100#", uri: nil, pin: nil)
        #expect(dialInURL(refused) == nil)
    }

    @Test func anEventWithAMeetingSaysSoToVoiceOver() {
        let plain = calendarEventLabel(title: "Sync", time: "09:00", calendar: "Work", participation: .accepted)
        let withMeeting = calendarEventLabel(
            title: "Sync", time: "09:00", calendar: "Work", participation: .accepted, hasConference: true
        )
        #expect(withMeeting == "\(plain), \(L10n.event_video_meeting())")
    }
}
