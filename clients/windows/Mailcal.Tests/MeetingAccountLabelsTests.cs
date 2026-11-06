// Settings → Videoconferencing's list of the accounts that make video meetings (docs/settings.md
// category 4): what each row says. The Windows twin of Apple's MeetingAccountsListTests.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class MeetingAccountLabelsTests
{
    private static readonly MeetingStateWords Words = new("On", "Off", "Needs permission", "Not offered");

    [Fact]
    public void EachStateIsSaidAsTheAccountPageMeansIt()
    {
        Assert.Equal("On", MeetingAccountLabels.State(CapabilityState.On, Words));
        Assert.Equal("Off", MeetingAccountLabels.State(CapabilityState.Off, Words));
        Assert.Equal("Needs permission", MeetingAccountLabels.State(CapabilityState.NeedsPermission, Words));
        Assert.Equal("Not offered", MeetingAccountLabels.State(CapabilityState.NotOffered, Words));
    }

    [Fact]
    public void AScreenReaderHearsTheServiceTheAccountAndTheState()
    {
        Assert.Equal(
            "Microsoft Teams, alice@work.example, Needs permission",
            MeetingAccountLabels.Spoken("Microsoft Teams", "alice@work.example", "Needs permission"));
    }
}
