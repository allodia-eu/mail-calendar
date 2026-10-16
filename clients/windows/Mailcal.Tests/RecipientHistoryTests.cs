// Undo and redo while writing (docs/undo.md): a recipient field's history driven directly, what
// one step is and the chords that move it; and the editor's right-click menu, which keeps the
// editing actions of WebView2's and draws Undo and Redo over the bundle's history.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class RecipientHistoryTests
{
    private const int KeyV = 0x56;
    private const int KeyY = 0x59;
    private const int KeyZ = 0x5A;

    // Types `text` one character at a time, as the input reports it.
    private static void Type(RecipientHistory history, string text)
    {
        foreach (var letter in text)
        {
            history.Record(history.Current + letter, typed: true);
        }
    }

    [Fact]
    public void OneAddressTypedIsOneStep()
    {
        var history = new RecipientHistory();
        Type(history, "bob@example.test");
        Assert.Equal(string.Empty, history.Undo());
    }

    // The step this exists for: undoing a finished address gives the text back, never a second pill.
    [Fact]
    public void FinishingAnAddressIsAStepOfItsOwn()
    {
        var history = new RecipientHistory();
        Type(history, "bob@example.test,");
        Type(history, " carol");
        Assert.Equal("bob@example.test,", history.Undo());
        Assert.Equal("bob@example.test", history.Undo());
    }

    [Fact]
    public void ARemovedRecipientComesBackAndGoesAgain()
    {
        var history = new RecipientHistory();
        history.Restart("bob@example.test, carol@example.test, ");
        history.Record("carol@example.test, ", typed: false);
        Assert.Equal("bob@example.test, carol@example.test, ", history.Undo());
        Assert.Equal("carol@example.test, ", history.Redo());
    }

    [Fact]
    public void AChangeAfterAnUndoLeavesNothingToRedo()
    {
        var history = new RecipientHistory();
        history.Record("bob@example.test, ", typed: false);
        history.Undo();
        history.Record("carol@example.test, ", typed: false);
        Assert.False(history.CanRedo);
        Assert.Null(history.Redo());
    }

    [Fact]
    public void TypingAfterAnUndoStartsANewStep()
    {
        var history = new RecipientHistory();
        Type(history, "bo");
        history.Undo();
        Type(history, "al");
        Assert.Equal(string.Empty, history.Undo());
        Assert.False(history.CanUndo);
        Assert.Null(history.Undo());
    }

    // A reply's recipients are where undo stops, not a step.
    [Fact]
    public void AValueFromOutsideTheFieldIsWhereItsHistoryStarts()
    {
        var history = new RecipientHistory();
        history.Record("bob@example.test, ", typed: false);
        history.Restart("ada@example.test, ");
        Assert.Null(history.Undo());
        Assert.Equal("ada@example.test, ", history.Current);
    }

    [Fact]
    public void TheNewestHundredStepsAreKept()
    {
        var history = new RecipientHistory();
        for (var index = 1; index <= 101; index++)
        {
            history.Record($"r{index}@example.test, ", typed: false);
        }
        var undone = 0;
        while (history.Undo() is not null)
        {
            undone++;
        }
        Assert.Equal(100, undone);
        Assert.Equal("r1@example.test, ", history.Current);
    }

    [Fact]
    public void TheChordsAreTheTextBoxsOwn()
    {
        Assert.Equal(HistoryStep.Undo, HistoryChord.Read(KeyZ, ctrl: true, shift: false, alt: false));
        Assert.Equal(HistoryStep.Redo, HistoryChord.Read(KeyZ, ctrl: true, shift: true, alt: false));
        Assert.Equal(HistoryStep.Redo, HistoryChord.Read(KeyY, ctrl: true, shift: false, alt: false));
        Assert.Null(HistoryChord.Read(KeyY, ctrl: true, shift: true, alt: false));
        Assert.Null(HistoryChord.Read(KeyZ, ctrl: false, shift: false, alt: false));
        // AltGr is Ctrl+Alt, and types a character on many layouts.
        Assert.Null(HistoryChord.Read(KeyZ, ctrl: true, shift: false, alt: true));
        Assert.Null(HistoryChord.Read(KeyV, ctrl: true, shift: false, alt: false));
    }

    [Theory]
    [InlineData("cut", true)]
    [InlineData("copy", true)]
    [InlineData("paste", true)]
    [InlineData("selectAll", true)]
    [InlineData("copyLinkLocation", true)]
    [InlineData("undo", false)]
    [InlineData("redo", false)]
    [InlineData("openLinkInNewWindow", false)]
    [InlineData("reload", false)]
    [InlineData("inspectElement", false)]
    [InlineData("other", false)]
    public void TheEditorMenuKeepsTheEditingActions(string name, bool kept) =>
        Assert.Equal(kept, EditorMenu.Keeps(name));

    [Theory]
    [InlineData("undo", "Undo")]
    [InlineData("redo", "Redo")]
    [InlineData("paste", null)]
    [InlineData("reload", null)]
    // The expected step by name: the enum is internal, and a public test cannot take it.
    public void TheEditorMenuReplacesUndoAndRedo(string name, string? step) =>
        Assert.Equal(step, EditorMenu.Step(name)?.ToString());

    [Fact]
    public void TheEditorMenuStepsTheBundlesHistory()
    {
        Assert.Equal("window.stepComposerHistory('undo')", EditorMenu.Script(HistoryStep.Undo));
        Assert.Equal("window.stepComposerHistory('redo')", EditorMenu.Script(HistoryStep.Redo));
    }

    [Theory]
    [InlineData("\"{\\\"undo\\\":true,\\\"redo\\\":false}\"", true, false)]
    [InlineData("\"{\\\"undo\\\":false,\\\"redo\\\":true}\"", false, true)]
    // The bundle has not parsed, is the fallback page, or did not answer in time: nothing is offered.
    [InlineData(null, false, false)]
    [InlineData("null", false, false)]
    [InlineData("\"not json\"", false, false)]
    [InlineData("\"[]\"", false, false)]
    [InlineData("", false, false)]
    public void TheEditorMenuReadsWhatTheBundleSaysItCanDo(string? encoded, bool undo, bool redo)
    {
        var steps = EditorMenu.ParseSteps(encoded);
        Assert.Equal((undo, redo), (steps.Contains(HistoryStep.Undo), steps.Contains(HistoryStep.Redo)));
    }
}
