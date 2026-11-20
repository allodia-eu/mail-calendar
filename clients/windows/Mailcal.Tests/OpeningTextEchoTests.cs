// A field's TextChanged for the value the composer opened it with is not an edit (docs/drafts.md,
// "When a save happens"). Counted as one, a reply, a forward or a resumed draft left open saves
// itself once the idle interval passes, with nothing typed.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class OpeningTextEchoTests
{
    [Fact]
    public void TheEchoOfTheOpeningSubjectIsNotAnEdit()
    {
        var echo = new OpeningTextEcho("Re: Terms");
        Assert.False(echo.IsEdit("Re: Terms"));
    }

    [Fact]
    public void EveryChangeAfterTheEchoIsAnEdit()
    {
        var echo = new OpeningTextEcho("Re: Terms");
        Assert.False(echo.IsEdit("Re: Terms"));
        Assert.True(echo.IsEdit("Re: Terms 2"));
        // Putting the opened value back is still the user's edit: a save may hold the other one.
        Assert.True(echo.IsEdit("Re: Terms"));
    }

    // An empty field assigned empty raises nothing, so the first change is a keystroke; and a
    // keystroke landing before the echo makes the echo carry the user's text.
    [Theory]
    [InlineData("", "R")]
    [InlineData("Re: Terms", "Re: Terms!")]
    public void AFirstChangeAwayFromTheOpeningValueIsAnEdit(string opened, string now) =>
        Assert.True(new OpeningTextEcho(opened).IsEdit(now));
}
