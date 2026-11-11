using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class DevBootRulesTests
{
    // The conformance suite runs against the demo account, and drives whatever it boots: one that
    // fell back to the stored accounts would be pressing controls over the developer's own mail.
    [Theory]
    [InlineData("demo")]
    [InlineData(" Demo ")]
    [InlineData("DEMO")]
    public void TheDemoAccountBootsTheInMemoryStore(string raw) =>
        Assert.Equal(DevBoot.Demo, DevBootRules.Resolve(raw));

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("personal")]
    [InlineData("stalwart")]
    [InlineData("first-run")]
    [InlineData("demos")]
    public void EverythingElseBootsTheAccounts(string? raw) =>
        Assert.Equal(DevBoot.Accounts, DevBootRules.Resolve(raw));

    // Its preferences are not the developer's either: a window resized in a demo run must not be
    // the size the real app opens at.
    [Fact]
    public void TheDemoAccountKeepsItsPreferencesApart() =>
        Assert.Equal(System.IO.Path.Combine("root", "dev-demo"), AppPaths.ResolvePrefsDir("root", "demo"));
}
