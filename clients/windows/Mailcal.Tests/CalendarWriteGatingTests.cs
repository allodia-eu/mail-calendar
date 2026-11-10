// The calendar write-capability gates, pinned without a UI.
//
// The policy is cross-client (Android and Apple carry the same suites): a per-event delete is
// HIDDEN when its record cannot write, and "New event" is DISABLED when no calendar on the page
// can, an empty calendar list (nothing synced yet) also disables it. The core stamps the flag;
// these tests pin that the client reads it off the exact record it renders. They are also the
// proof of the read-only branch: the local harness has no read-only account to exercise it against.
using System.Linq;
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarWriteGatingTests
{
    private static CalendarRow Calendar(string id, bool canWrite)
    {
        var swatch = new Swatch("#336699", "#ffffff", "#224466");
        return new CalendarRow(
            "acct-1", "me@example.org", id, "Calendar " + id, new CalendarColor("#336699", swatch, swatch),
            true, canWrite, false, canWrite);
    }

    // `participation` is inert here, write-gating reads `can_write` and nothing else, so these
    // rows are Accepted rather than NeedsAction, which would additionally make them unanswered
    // holds and put a second variable in a single-variable test.
    private static EventRow Event(bool canWrite) =>
        new("acct-1", "ev-1", "Standup", "2026-07-16 09:00", canWrite, ResponseStatus.Accepted, false);

    [Fact]
    public void New_event_needs_at_least_one_writable_calendar()
    {
        Assert.True(CalendarWriteGating.CanCreate([Calendar("a", canWrite: false), Calendar("b", canWrite: true)]));
        Assert.False(CalendarWriteGating.CanCreate([Calendar("a", canWrite: false), Calendar("b", canWrite: false)]));
    }

    private static CalendarRow CalendarOf(string account, string id, bool canWrite = true)
    {
        var swatch = new Swatch("#336699", "#ffffff", "#224466");
        return new CalendarRow(
            account, account + "@test.local", id, id, new CalendarColor("#336699", swatch, swatch),
            true, canWrite, false, false);
    }

    [Fact]
    public void The_default_is_chosen_among_writable_calendars_only()
    {
        var groups = CalendarWriteGating.DefaultChoices(
            [CalendarOf("a", "work"), CalendarOf("a", "holidays", canWrite: false)]);
        var group = Assert.Single(groups);
        Assert.Null(group.Header);
        Assert.Equal(["work"], group.Rows.Select(row => row.Id));
    }

    [Fact]
    public void Two_accounts_are_two_groups_named_by_address()
    {
        var groups = CalendarWriteGating.DefaultChoices(
            [CalendarOf("a", "work"), CalendarOf("b", "work"), CalendarOf("a", "home")]);
        Assert.Equal(["a@test.local", "b@test.local"], groups.Select(group => group.Header));
        Assert.Equal(["work", "home"], groups[0].Rows.Select(row => row.Id));
    }

    [Fact]
    public void No_calendars_synced_means_no_new_event()
    {
        Assert.False(CalendarWriteGating.CanCreate([]));
    }

    [Fact]
    public void Delete_shows_only_on_a_writable_row()
    {
        Assert.True(CalendarWriteGating.OffersDelete(Event(canWrite: true)));
        Assert.False(CalendarWriteGating.OffersDelete(Event(canWrite: false)));
    }
}
