// The attendee row's text, tested without a dialog. The rule is small and entirely about what the
// user reads, which is exactly the kind that quietly regresses: the second line exists to add
// something the first line does not already say, so an unnamed attendee must not get their own
// address printed twice.
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AttendeeSummaryTests
{
    private const string Organizer = "Organiser";

    private static EventAttendee Attendee(string name, string email, bool isOrganizer = false) =>
        new(name, email, isOrganizer, ResponseStatus.Accepted);

    [Fact]
    public void ANamedAttendeeIsShownByNameWithTheAddressBeneath()
    {
        var attendee = Attendee("Anna Jansen", "anna@example.com");
        Assert.Equal("Anna Jansen", AttendeeSummary.Title(attendee));
        Assert.Equal("anna@example.com", AttendeeSummary.Subtitle(attendee, Organizer));
    }

    [Fact]
    public void AnUnnamedAttendeeIsShownByAddressAndHasNoSecondLine()
    {
        // Printing the address again underneath itself is the failure this pins.
        var attendee = Attendee(string.Empty, "b@example.com");
        Assert.Equal("b@example.com", AttendeeSummary.Title(attendee));
        Assert.Equal(string.Empty, AttendeeSummary.Subtitle(attendee, Organizer));
    }

    [Fact]
    public void AnUnnamedOrganizerStillSaysTheyCalledTheMeeting()
    {
        var attendee = Attendee(string.Empty, "chair@example.com", isOrganizer: true);
        Assert.Equal(Organizer, AttendeeSummary.Subtitle(attendee, Organizer));
    }

    [Fact]
    public void ANamedOrganizerShowsBothTheAddressAndTheRole()
    {
        var attendee = Attendee("Chair Person", "chair@example.com", isOrganizer: true);
        Assert.Equal("chair@example.com · Organiser", AttendeeSummary.Subtitle(attendee, Organizer));
    }

    [Fact]
    public void TheEditorCarriesTheAttendeesThroughSoTheyCanBeShownReadOnly()
    {
        // The editor prefills from the same detail read; without this the list would be empty in
        // the editor while the detail sheet showed it, on the same event.
        var attendees = new[] { Attendee("Anna Jansen", "anna@example.com") };
        var detail = new EventDetail(
            "acct", "/cal/e.ics", "work", "Standup", false, "Europe/Amsterdam",
            "2026-01-05T09:30:00", "2026-01-05T10:00:00", null, null, null, null, null, null, false,
            true, string.Empty, attendees);

        var editor = EventEditorState.Edit(detail, "Work");

        Assert.Equal(attendees, editor.Editing!.Attendees);
    }
}
