// The delete gate, pinned without a UI: a per-event delete is HIDDEN when its record cannot write.
// The core stamps the flag; this pins that the client reads it off the exact record it renders,
// and is the proof of the read-only branch, which the local harness has no account to exercise.
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarWriteGatingTests
{
    // `participation` is inert here, write-gating reads `can_write` and nothing else, so these
    // rows are Accepted rather than NeedsAction, which would additionally make them unanswered
    // holds and put a second variable in a single-variable test.
    private static EventRow Event(bool canWrite) =>
        new("acct-1", "ev-1", "Standup", "2026-07-16 09:00", canWrite, ResponseStatus.Accepted, false);

    [Fact]
    public void Delete_shows_only_on_a_writable_row()
    {
        Assert.True(CalendarWriteGating.OffersDelete(Event(canWrite: true)));
        Assert.False(CalendarWriteGating.OffersDelete(Event(canWrite: false)));
    }
}
