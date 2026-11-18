// The Settings signature editor's half of the page the core draws (docs/signatures.md, "Changing
// the library"). Which outcome a change came to is the core's, pinned in mailcal-bindings'
// signatures_page_tests.rs and app_signatures_tests.rs; what is pinned here is what the editor does
// with it: the id it saves under, when it closes, when it stays and what it says.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SignatureEditorTests
{
    private static readonly ControlState Free = new(true, false);
    private static readonly ControlState Running = new(false, true);
    private static readonly ControlState Held = new(false, false);

    private static readonly SignatureRefusalWords Words = new(
        "name required",
        "gone",
        "png, jpeg or gif",
        limit => $"under {limit}");

    private static SignaturesPage Page(SignatureOutcome? outcome = null, string newId = "new-1") =>
        new([Entry("work")], [], Free, newId, Free, outcome);

    private static SignatureEntry Entry(string id) => new(id, "Work", Free, Free);

    private static SignatureOutcome Outcome(ulong serial, string id, SignatureResult result) =>
        new(serial, id, SignatureChangeKind.Save, result);

    [Fact]
    public void ANewSignatureSavesUnderThePagesNewId() =>
        Assert.Equal("new-1", SignatureEditorSession.New(Page()).Id);

    [Fact]
    public void AnEditSavesUnderTheEntrysId() =>
        Assert.Equal("work", SignatureEditorSession.Edit(Entry("work"), Page()).Id);

    // The id is the one in force when the editor opened: after the first save of a new signature
    // the page offers another new id, and a second press must save the same signature again.
    [Fact]
    public void ANewSignatureKeepsItsIdWhenThePageMintsAnother()
    {
        var session = SignatureEditorSession.New(Page(newId: "new-1"));
        Assert.Equal(SignatureEditorStep.Refused, session.Read(
            Outcome(1, "new-1", new SignatureResult.Refused(new SignatureRefusal.BlankName())), out _));
        Assert.Equal("new-1", session.Id);
    }

    // The generated results are internal, so a theory names them rather than taking them.
    [Theory]
    [InlineData("saved")]
    [InlineData("deleted")]
    [InlineData("not written")]
    public void SavedDeletedAndNotWrittenCloseTheEditor(string came)
    {
        SignatureResult result = came switch
        {
            "saved" => new SignatureResult.Saved(),
            "deleted" => new SignatureResult.Deleted(),
            _ => new SignatureResult.NotWritten(),
        };
        var session = SignatureEditorSession.Edit(Entry("work"), Page());
        Assert.Equal(SignatureEditorStep.Close, session.Read(Outcome(1, "work", result), out var reason));
        Assert.Null(reason);
    }

    [Fact]
    public void ARefusalKeepsTheEditorOpenWithItsReason()
    {
        var session = SignatureEditorSession.Edit(Entry("work"), Page());
        var gone = new SignatureRefusal.Gone();
        Assert.Equal(SignatureEditorStep.Refused, session.Read(
            Outcome(1, "work", new SignatureResult.Refused(gone)), out var reason));
        Assert.Equal(gone, reason);
    }

    // The outcome in force when the editor opened belongs to an earlier change: acting on it
    // would close an editor the person has only just opened.
    [Fact]
    public void TheOutcomeInForceWhenItOpenedIsNotActedOn()
    {
        var earlier = Outcome(4, "work", new SignatureResult.Saved());
        var session = SignatureEditorSession.Edit(Entry("work"), Page(earlier));
        Assert.Equal(SignatureEditorStep.Stay, session.Read(earlier, out _));
    }

    // The page is read on every signal, and the same outcome stays on it until the next change.
    [Fact]
    public void AnOutcomeIsActedOnOnce()
    {
        var session = SignatureEditorSession.Edit(Entry("work"), Page());
        var refused = Outcome(1, "work", new SignatureResult.Refused(new SignatureRefusal.Gone()));
        Assert.Equal(SignatureEditorStep.Refused, session.Read(refused, out _));
        Assert.Equal(SignatureEditorStep.Stay, session.Read(refused, out _));
    }

    [Fact]
    public void AnotherSignaturesOutcomeLeavesTheEditorAlone()
    {
        var session = SignatureEditorSession.Edit(Entry("work"), Page());
        Assert.Equal(SignatureEditorStep.Stay, session.Read(Outcome(1, "home", new SignatureResult.Deleted()), out _));
        Assert.Equal(SignatureEditorStep.Stay, session.Read(null, out _));
    }

    [Fact]
    public void SaveIsThePagesWhileTheNameIsAccepted()
    {
        Assert.Equal(Free, SignatureEditorSession.SaveState(Free, null));
        Assert.Equal(Running, SignatureEditorSession.SaveState(Running, null));
    }

    [Fact]
    public void SaveIsHeldWhileTheNameIsRefused()
    {
        var blank = new SignatureRefusal.BlankName();
        Assert.Equal(Held, SignatureEditorSession.SaveState(Free, blank));
        Assert.Equal(Running, SignatureEditorSession.SaveState(Running, blank));
    }

    [Fact]
    public void EachRefusalIsWordedByItsOwnKey()
    {
        Assert.Equal("name required", SignatureEditorSession.Say(new SignatureRefusal.BlankName(), Words));
        Assert.Equal("gone", SignatureEditorSession.Say(new SignatureRefusal.Gone(), Words));
        Assert.Equal("png, jpeg or gif", SignatureEditorSession.Say(
            new SignatureRefusal.Image(new SignatureImageRefusal.Format()), Words));
        Assert.Equal("png, jpeg or gif", SignatureEditorSession.Say(new SignatureImageRefusal.Format(), Words));
    }

    // The limit is the core's, not a constant of this client's: a different one is said as given.
    [Theory]
    [InlineData(512UL * 1024, "under 512 KB")]
    [InlineData(2UL * 1024 * 1024, "under 2 MB")]
    public void ATooLargePictureNamesTheCoresLimit(ulong limit, string said)
    {
        Assert.Equal(said, SignatureEditorSession.Say(new SignatureImageRefusal.TooLarge(limit), Words));
        Assert.Equal(said, SignatureEditorSession.Say(
            new SignatureRefusal.Image(new SignatureImageRefusal.TooLarge(limit)), Words));
    }
}
