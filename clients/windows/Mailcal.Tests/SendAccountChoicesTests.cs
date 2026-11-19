// Settings → Composing's default send account, drawn from the core's SendAccountSetting: which
// changes redraw the row rather than move the picker in place, and which account it opens on.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SendAccountChoicesTests
{
    private static SendAccountSetting.Choose Choose(string selected, params string[] ids) =>
        new(ids.Select(id => new SendAccountOption(id, id + "@test.local")).ToArray(), selected);

    [Fact]
    public void AnotherDefaultMovesThePickerRatherThanRedrawing()
    {
        Assert.Equal(
            SendAccountChoices.Fingerprint(Choose("work", "work", "home")),
            SendAccountChoices.Fingerprint(Choose("home", "work", "home")));
    }

    [Fact]
    public void AnAccountArrivingOrLeavingRedraws()
    {
        var two = SendAccountChoices.Fingerprint(Choose("work", "work", "home"));
        Assert.NotEqual(two, SendAccountChoices.Fingerprint(Choose("work", "work", "home", "club")));
        Assert.NotEqual(two, SendAccountChoices.Fingerprint(new SendAccountSetting.Only("work@test.local")));
        Assert.NotEqual(
            SendAccountChoices.Fingerprint(new SendAccountSetting.Only("work@test.local")),
            SendAccountChoices.Fingerprint(new SendAccountSetting.NoAccount()));
    }

    [Fact]
    public void ThePickerOpensOnTheAccountTheCoreNames()
    {
        Assert.Equal(1, SendAccountChoices.SelectedIndex(Choose("home", "work", "home")));
        Assert.Equal(0, SendAccountChoices.SelectedIndex(Choose("work", "work", "home")));
    }
}
