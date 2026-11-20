// Draft a reply's suggestions and remedies as the composer takes them (docs/ai.md rules 18 and 22).
// Which suggestions and which remedy are the core's, pinned in mailcal-app; what is pinned here is
// that each suggestion is worded by its own key and each remedy leads where it can be taken.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ReplyDraftWordsTests
{
    private static readonly ReplyDraftWords Words = new("yes", "no", "more information", "holding", gap => $"fill in {gap}");

    [Fact]
    public void AGapToFillInIsNamedAsTheReplyCarriesItAndAnyOtherItemAsWritten()
    {
        Assert.Equal("fill in [date]", Words.Of(new ReplyDraftItem(0, ReplyDraftTaskKind.FillIn, "[date]", false, null)));
        Assert.Equal("Attach the agenda", Words.Of(new ReplyDraftItem(1, ReplyDraftTaskKind.Attach, "Attach the agenda", false, null)));
        Assert.Equal("Book a room", Words.Of(new ReplyDraftItem(2, ReplyDraftTaskKind.Do, "Book a room", false, new ControlState(true, false))));
    }

    [Fact]
    public void EachSuggestionIsWordedByItsOwnKey()
    {
        Assert.Equal("yes", Words.Of(ReplyDraftSuggestion.Yes));
        Assert.Equal("no", Words.Of(ReplyDraftSuggestion.No));
        Assert.Equal("more information", Words.Of(ReplyDraftSuggestion.MoreInformation));
        Assert.Equal("holding", Words.Of(ReplyDraftSuggestion.Holding));
    }

    [Fact]
    public void TryAgainStaysInTheComposerAndSignInOpensOverIt()
    {
        Assert.Equal(ReplyDraftRemedyPlace.Retry, ReplyDraftRemedies.PlaceOf(AiRemedy.TryAgain));
        Assert.Equal(ReplyDraftRemedyPlace.SignIn, ReplyDraftRemedies.PlaceOf(AiRemedy.SignInToAllodia));
    }

    [Fact]
    public void ARemedyTheComposerCannotTakeOpensSettingsWhereItCanBe()
    {
        Assert.Equal(ReplyDraftRemedyPlace.Endpoint, ReplyDraftRemedies.PlaceOf(AiRemedy.OpenAiEndpoint));
        Assert.Equal(ReplyDraftRemedyPlace.Learn, ReplyDraftRemedies.PlaceOf(AiRemedy.LearnStyle));
        Assert.Equal(ReplyDraftRemedyPlace.WritingStyle, ReplyDraftRemedies.PlaceOf(AiRemedy.ChooseStyle));
    }

    [Fact]
    public void EveryRemedyLeadsSomewhere()
    {
        foreach (var remedy in Enum.GetValues<AiRemedy>())
        {
            Assert.True(Enum.IsDefined(ReplyDraftRemedies.PlaceOf(remedy)));
        }
    }
}
