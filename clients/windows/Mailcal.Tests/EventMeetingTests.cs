// What the event detail says about a video meeting, and that its links are the core's as sent,
// tested without a dialog (docs/conferencing.md). The Windows twin of Apple's EventConferenceTests.
// Which links those are is decided and pinned in the core (event_conference_tests.rs).
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class EventMeetingTests
{
    private const string Teams = "https://teams.microsoft.com/meet/123?p=x";

    private static EventConference Meeting(
        string? service = "Microsoft Teams",
        string? joinUrl = Teams,
        ConferenceState state = ConferenceState.Ready) =>
        new(service, joinUrl, "123 456", null, [], null, state);

    [Fact]
    public void A_meeting_is_headed_by_its_service_and_otherwise_by_the_catalogs_words()
    {
        Assert.Equal("Microsoft Teams", EventMeeting.Title(Meeting(), "Video meeting"));
        Assert.Equal("Video meeting", EventMeeting.Title(Meeting(service: null), "Video meeting"));
        Assert.Equal("Video meeting", EventMeeting.Title(Meeting(service: string.Empty), "Video meeting"));
    }

    [Fact]
    public void Join_opens_the_cores_link_as_sent()
    {
        Assert.Equal(new Uri(Teams), EventMeeting.JoinUri(Meeting()));
        Assert.Null(EventMeeting.JoinUri(Meeting(joinUrl: null)));
    }

    [Fact]
    public void A_meeting_still_being_made_says_so_instead_of_offering_join()
    {
        Assert.Null(EventMeeting.Status(Meeting(), "Preparing", "Failed"));
        Assert.Equal("Preparing", EventMeeting.Status(Meeting(state: ConferenceState.Preparing), "Preparing", "Failed"));
        Assert.Equal("Failed", EventMeeting.Status(Meeting(state: ConferenceState.Failed), "Preparing", "Failed"));
    }

    [Fact]
    public void A_dial_in_row_opens_the_cores_number_and_a_refused_one_is_text()
    {
        var number = new ConferenceDialIn("+31 20 555 0100", "tel:+31205550100", "990#");
        Assert.Equal("tel:+31205550100", EventMeeting.DialInUri(number)?.OriginalString);
        Assert.Null(EventMeeting.DialInUri(new ConferenceDialIn("*21*0100#", null, null)));
    }

    [Fact]
    public void A_row_with_no_value_is_left_out()
    {
        Assert.False(EventMeeting.Shows(null));
        Assert.False(EventMeeting.Shows(string.Empty));
        Assert.True(EventMeeting.Shows("123 456"));
    }

    [Fact]
    public void An_event_with_a_meeting_says_so_to_a_screen_reader()
    {
        Assert.Equal("Sync, 09:00, Work", EventMeeting.Spoken("Sync, 09:00, Work", "Video meeting", false));
        Assert.Equal(
            "Sync, 09:00, Work, Video meeting",
            EventMeeting.Spoken("Sync, 09:00, Work", "Video meeting", true));
    }
}
