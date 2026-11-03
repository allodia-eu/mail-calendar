// What the event detail says about a video meeting, and what its links open, tested without a
// dialog (docs/conferencing.md). The Windows twin of Apple's EventConferenceTests. The links matter
// most: Join and a dial-in number are the two places a press leaves the app.
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

    // The shared policy admits web and mail links; the core's own answer is not loadable here.
    private static bool WebOrMail(string url) =>
        url.StartsWith("https://", StringComparison.OrdinalIgnoreCase)
        || url.StartsWith("http://", StringComparison.OrdinalIgnoreCase)
        || url.StartsWith("mailto:", StringComparison.OrdinalIgnoreCase);

    [Fact]
    public void A_meeting_is_headed_by_its_service_and_otherwise_by_the_catalogs_words()
    {
        Assert.Equal("Microsoft Teams", EventMeeting.Title(Meeting(), "Video meeting"));
        Assert.Equal("Video meeting", EventMeeting.Title(Meeting(service: null), "Video meeting"));
        Assert.Equal("Video meeting", EventMeeting.Title(Meeting(service: string.Empty), "Video meeting"));
    }

    [Fact]
    public void Join_opens_the_cores_web_link_and_nothing_else()
    {
        Assert.Equal(new Uri(Teams), EventMeeting.JoinUri(Meeting(), WebOrMail));
        Assert.Null(EventMeeting.JoinUri(Meeting(joinUrl: null), WebOrMail));
        Assert.Null(EventMeeting.JoinUri(Meeting(joinUrl: "javascript:alert(1)"), WebOrMail));
        // Admitted by the policy, but a meeting is joined over https only.
        Assert.Null(EventMeeting.JoinUri(Meeting(joinUrl: "http://example.com/meet"), WebOrMail));
        Assert.Null(EventMeeting.JoinUri(Meeting(joinUrl: "mailto:a@example.com"), WebOrMail));
    }

    [Fact]
    public void Join_asks_the_shared_link_policy_first() =>
        Assert.Null(EventMeeting.JoinUri(Meeting(), _ => false));

    [Fact]
    public void A_meeting_still_being_made_says_so_instead_of_offering_join()
    {
        Assert.Null(EventMeeting.Status(Meeting(), "Preparing", "Failed"));
        Assert.Equal("Preparing", EventMeeting.Status(Meeting(state: ConferenceState.Preparing), "Preparing", "Failed"));
        Assert.Equal("Failed", EventMeeting.Status(Meeting(state: ConferenceState.Failed), "Preparing", "Failed"));
        // Nothing to press, whatever link a server left on it.
        Assert.Null(EventMeeting.JoinUri(Meeting(state: ConferenceState.Preparing), WebOrMail));
        Assert.Null(EventMeeting.JoinUri(Meeting(state: ConferenceState.Failed), WebOrMail));
    }

    [Fact]
    public void A_dial_in_row_opens_only_a_telephone_number()
    {
        var number = new ConferenceDialIn("+31 20 555 0100", "tel:+31205550100", "990#");
        Assert.Equal("tel:+31205550100", EventMeeting.DialInUri(number)?.OriginalString);
        Assert.Null(EventMeeting.DialInUri(new ConferenceDialIn("x", "https://example.com/", null)));
        Assert.Null(EventMeeting.DialInUri(new ConferenceDialIn("x", "+31205550100", null)));
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
