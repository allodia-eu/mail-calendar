// Which dev builds' handlers of the dev scheme a launch withdraws. Every one left behind is a row in
// the "select an app" picker a sign-in redirect opens, and every row but one hands the
// authorisation code to a build with no sign-in waiting.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public sealed class DevSchemeOwnershipTests
{
    private const string Own = @"D:\repos\mail-calendar\clients\windows\Mailcal\bin\ARM64\Debug\win-arm64\Mailcal.exe";

    [Fact]
    public void EveryOtherBuildIsWithdrawnAndThisOneIsKept()
    {
        var others = DevSchemeOwnership.OthersToWithdraw(
            [Own, @"D:\repos\wt\a\Mailcal.exe", @"D:\gone\Mailcal.exe"], Own);

        Assert.Equal([@"D:\repos\wt\a\Mailcal.exe", @"D:\gone\Mailcal.exe"], others);
    }

    [Fact]
    public void ThisBuildIsRecognisedWhateverTheCaseOfItsPath()
    {
        // The build script writes bin\ARM64 and an older one wrote bin\arm64: the same folder.
        var others = DevSchemeOwnership.OthersToWithdraw([Own.ToUpperInvariant()], Own);
        Assert.Empty(others);
    }

    [Fact]
    public void EachOtherBuildIsWithdrawnOnce()
    {
        var others = DevSchemeOwnership.OthersToWithdraw(
            [@"D:\wt\Mailcal.exe", @"d:\WT\Mailcal.exe"], Own);
        Assert.Single(others);
    }

    [Fact]
    public void ARegistrationNamingNoExeIsLeftAlone()
    {
        // The only way to withdraw one is by the exe it was made for.
        var others = DevSchemeOwnership.OthersToWithdraw([null, "", "  "], Own);
        Assert.Empty(others);
    }

    [Theory]
    [InlineData("\"D:\\a b\\Mailcal.exe\" \"----ms-protocol:%1\"", "D:\\a b\\Mailcal.exe")]
    [InlineData("D:\\a\\Mailcal.exe ----ms-protocol:%1", "D:\\a\\Mailcal.exe")]
    [InlineData("D:\\a\\Mailcal.exe", "D:\\a\\Mailcal.exe")]
    [InlineData("\"unterminated", null)]
    [InlineData("", null)]
    [InlineData(null, null)]
    public void TheExeIsReadFromTheOpenCommand(string? command, string? exe)
    {
        Assert.Equal(exe, DevSchemeOwnership.ExeFromCommand(command));
    }
}
