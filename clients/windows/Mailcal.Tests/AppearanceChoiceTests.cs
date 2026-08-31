// Which appearance a launch paints in. Two properties matter and neither is visible on screen:
// the MAILCAL_APPEARANCE override must beat the stored choice (that is the whole point of it, a
// showcase or UI run photographs both themes without touching the developer's desktop), and a
// spelling it does not know must fall through to the stored choice rather than quietly meaning
// "system". A silently-ignored override looks exactly like a working one in a screenshot.
//
// The expectations sit in the bodies rather than in [InlineData]: the generated bindings' types are
// `internal`, so an `Appearance` in a public test signature is a CS0051 accessibility error.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AppearanceChoiceTests
{
    [Theory]
    [InlineData("light")]
    [InlineData("Light")]
    [InlineData(" LIGHT ")]   // trimmed + case-insensitive, like every other launch hook
    public void ALightOverrideBeatsTheStoredChoice(string raw) =>
        Assert.Equal(Appearance.Light, AppearanceChoice.Resolve(raw, Appearance.Dark));

    [Theory]
    [InlineData("dark")]
    [InlineData("DARK")]
    public void ADarkOverrideBeatsTheStoredChoice(string raw) =>
        Assert.Equal(Appearance.Dark, AppearanceChoice.Resolve(raw, Appearance.Light));

    // "system" is an override in its own right, not an absent one: it forces a run to follow the
    // desktop even for a developer whose stored choice is Light or Dark.
    [Fact]
    public void SystemIsAnOverrideRatherThanAnAbsentOne() =>
        Assert.Equal(Appearance.System, AppearanceChoice.Resolve("system", Appearance.Dark));

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("  ")]
    [InlineData("night")]
    [InlineData("1")]
    public void AnythingElseLeavesTheStoredChoiceStanding(string? raw)
    {
        Assert.Equal(Appearance.Light, AppearanceChoice.Resolve(raw, Appearance.Light));
        Assert.Equal(Appearance.System, AppearanceChoice.Resolve(raw, Appearance.System));
    }
}
