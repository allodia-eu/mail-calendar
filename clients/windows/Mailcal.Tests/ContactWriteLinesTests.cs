// The line beside the contacts list for the most recent save (docs/contacts.md §3).

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ContactWriteLinesTests
{
    [Fact]
    public void Every_status_maps_to_its_line()
    {
        Assert.Equal(ContactWriteLine.None, ContactWriteLines.Of(ContactWriteStatus.Idle));
        Assert.Equal(ContactWriteLine.Saving, ContactWriteLines.Of(ContactWriteStatus.Saving));
        Assert.Equal(ContactWriteLine.Saved, ContactWriteLines.Of(ContactWriteStatus.Saved));
        Assert.Equal(ContactWriteLine.NotSaved, ContactWriteLines.Of(ContactWriteStatus.NotSaved));
        Assert.Equal(ContactWriteLine.Unconfirmed, ContactWriteLines.Of(ContactWriteStatus.Unconfirmed));
        // Stated under the form the user is still looking at, and nowhere else.
        Assert.Equal(ContactWriteLine.None, ContactWriteLines.Of(ContactWriteStatus.Invalid));
    }

    [Fact]
    public void Every_status_the_core_has_is_mapped()
    {
        var mapped = new[]
        {
            ContactWriteStatus.Idle, ContactWriteStatus.Saving, ContactWriteStatus.Saved,
            ContactWriteStatus.NotSaved, ContactWriteStatus.Unconfirmed, ContactWriteStatus.Invalid,
        };
        Assert.Equal(mapped, Enum.GetValues<ContactWriteStatus>());
    }
}
