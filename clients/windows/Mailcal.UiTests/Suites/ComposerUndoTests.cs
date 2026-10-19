// Undo and redo while writing (docs/undo.md): the recipient fields' own history, and the composer's
// right-click menus, whose Undo and Redo reach the same history as the chords. Mailcal.Tests holds
// the step boundaries (RecipientHistoryTests) and which of WebView2's items the editor keeps
// (EditorMenu), and nothing about whether either is wired: whether the field hears a chord ahead of
// its text box, whether the box's own history is kept out, whether the menus draw the field's and
// the bundle's Undo at all.
//
// The keys are real, since the history is made of keystrokes. Menu items are clicked, never invoked:
// an item in a TextCommandBarFlyout answers Invoke without running its command. The body and its
// menu are WebView2's own windows, in the runtime's process, so they are read through UI
// Automation, and the menu's Undo and Redo are its first two items, where WebView2 draws its own,
// since their labels are the system's. Nothing is sent; each composer is discarded.

using System.Diagnostics;
using System.Windows.Automation;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class ComposerUndoTests(ShowcaseApp showcase)
{
    private const string Address = "bob@example.test";
    private const ushort Z = 0x5A;
    private const ushort Y = 0x59;
    private const ushort F10 = 0x79;

    private void Case(string name, Action<App, Element> body) =>
        showcase.Case(name, app =>
        {
            new DraftsFlow(app).OpenNewComposer();
            body(app, app.Agent.Find(Filter.ById("ToField") with { OnScreen = true }) ?? throw new InvalidOperationException("no ToField on screen"));
        }, app =>
        {
            Keys(app, Input.Escape);
            app.CloseComposer();
        });

    private static void Keys(App app, ushort key, params ushort[] modifiers)
    {
        Input.BringToFront(app.Mailbox.Hwnd);
        Input.Chord(key, modifiers);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(300));
    }

    /// <summary>Puts the caret in <paramref name="field"/> and types a character at a time: sent in
    /// one burst, the characters reach the input before it reports any, and it reports them as one
    /// change, which the field rightly files as one step, as it would a paste.</summary>
    private static void TypeInto(App app, Element field, string text)
    {
        app.ClickToType(field);
        foreach (var character in text)
        {
            Input.Type(character.ToString());
            Thread.Sleep(40);
        }
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(400));
    }

    private static List<Element> Pills(App app) => app.Agent.FindAll(new Filter { Type = "Button", NameLike = "*@example.test*" }).ToList();

    private static string Value(App app, Element field) => app.Agent.Read(field).Value ?? "";

    /// <summary>The WebView2 runtime's own top-level windows, where the page and its menu are.</summary>
    private static IEnumerable<AutomationElement> BrowserWindows() =>
        AutomationElement.RootElement.FindAll(TreeScope.Children, Condition.TrueCondition).Cast<AutomationElement>()
            .Where(w => w.Current.ClassName.StartsWith("Chrome_WidgetWin", StringComparison.Ordinal) && ProcessName(w.Current.ProcessId) == "msedgewebview2");

    private static string ProcessName(int id)
    {
        try
        {
            using var process = Process.GetProcessById(id);
            return process.ProcessName;
        }
        catch (ArgumentException)
        {
            return "";
        }
    }

    /// <summary>The body as UI Automation reads it: the editor element, named by its placeholder,
    /// whose text pattern is the body's text. Waited for: the page loads after the composer.</summary>
    private static AutomationElement Body()
    {
        var named = new PropertyCondition(AutomationElement.NameProperty, Catalog.English("editor_placeholder"));
        AutomationElement? body = null;
        return App.Eventually(() => (body = BrowserWindows().Select(w => w.FindFirst(TreeScope.Descendants, named)).FirstOrDefault(b => b is not null)) is not null, TimeSpan.FromSeconds(15))
            ? body!
            : throw new InvalidOperationException("the editor exposes no body within 15 s: has its page loaded?");
    }

    private static string BodyText() => ((TextPattern)Body().GetCurrentPattern(TextPattern.Pattern)).DocumentRange.GetText(-1);

    /// <summary>Clicks the body's first line, the empty one a new message opens with.</summary>
    private static void ClickBody(App app, bool right = false)
    {
        var bounds = Body().Current.BoundingRectangle;
        var scale = Input.Scale(app.Mailbox.Hwnd);
        Input.BringToFront(app.Mailbox.Hwnd);
        Input.ClickAt((int)(bounds.X + (24 * scale)), (int)(bounds.Y + (14 * scale)), right: right);
        Thread.Sleep(1200);
    }

    /// <summary>WebView2's right-click menu, top to bottom.</summary>
    private static List<AutomationElement> BodyMenu() =>
        BrowserWindows().SelectMany(w => w.FindAll(TreeScope.Descendants, new PropertyCondition(AutomationElement.ControlTypeProperty, ControlType.MenuItem)).Cast<AutomationElement>())
            .OrderBy(item => item.Current.BoundingRectangle.Y).ToList();

    private static void Click(System.Windows.Rect bounds)
    {
        Input.ClickAt((int)(bounds.X + (bounds.Width / 2)), (int)(bounds.Y + (bounds.Height / 2)));
        Thread.Sleep(600);
    }

    [OfflineFact]
    public void AFinishedAddressUndoesToTheTextThatWasTypedAndRedoesToItsPill() => Case(nameof(AFinishedAddressUndoesToTheTextThatWasTypedAndRedoesToItsPill), (app, to) =>
    {
        TypeInto(app, to, $"{Address},");
        Assert.True(Pills(app).Count == 1, "a comma finishes an address into a pill");
        Keys(app, Z, Input.Control);
        Assert.Equal(Address, Value(app, to));
        Assert.True(Pills(app).Count == 0, "Ctrl+Z after finishing an address takes its pill away rather than finishing it again");
        Keys(app, Y, Input.Control);
        Assert.True(Pills(app).Count == 1, "Ctrl+Y puts the pill back");
        Keys(app, Z, Input.Control);
        Keys(app, Z, Input.Control, Input.Shift);
        Assert.True(Pills(app).Count == 1, "Shift+Ctrl+Z redoes as Ctrl+Y does");
        Assert.Equal("", Value(app, to));
    });

    [OfflineFact]
    public void ARemovedRecipientComesBackWithCtrlZFromTheFieldTheCaretWasPutBackIn() => Case(nameof(ARemovedRecipientComesBackWithCtrlZFromTheFieldTheCaretWasPutBackIn), (app, to) =>
    {
        TypeInto(app, to, $"{Address}, carol@example.test,");
        Assert.True(Pills(app).Count == 2, "two finished addresses are two pills");
        app.Agent.Invoke(Pills(app).First(p => p.Name.StartsWith(Address, StringComparison.Ordinal)));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
        Assert.True(Pills(app).Count == 1, "the remove button took its pill away");
        Assert.True(app.Agent.Read(to).Focused, "removing a pill puts the caret back in the field, where the chord that brings it back is heard");
        Keys(app, Z, Input.Control);
        Assert.True(Pills(app).Count == 2, "Ctrl+Z brings the removed recipient back");
    });

    [OfflineFact]
    public void TwoQuickUndosAreTwoStepsAndBothRedo() => Case(nameof(TwoQuickUndosAreTwoStepsAndBothRedo), (app, to) =>
    {
        TypeInto(app, to, $"{Address}, carol@example.test,");
        Assert.True(Pills(app).Count == 2, "two finished addresses are two pills");
        // One burst, so the second undo lands before the input reports the first.
        Input.BringToFront(app.Mailbox.Hwnd);
        Input.Chord(Z, Input.Control);
        Input.Chord(Z, Input.Control);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(400));
        Assert.True(Pills(app).Count < 2, "two undos take at least one address back");
        Keys(app, Y, Input.Control);
        Keys(app, Y, Input.Control);
        Assert.True(Pills(app).Count == 2, "the field writing its own step into the input is not filed as typing, which would clear what there is to redo");
        Assert.Equal("", Value(app, to));
    });

    [OfflineFact]
    public void TheToFieldsRightClickMenuUndoesAndRedoesTheFieldsOwnSteps() => Case(nameof(TheToFieldsRightClickMenuUndoesAndRedoesTheFieldsOwnSteps), (app, to) =>
    {
        TypeInto(app, to, $"{Address},");
        Keys(app, F10, Input.Shift);
        var undo = app.Agent.Wait(Filter.ById("RecipientUndo") with { OnScreen = true }, TimeSpan.FromSeconds(5))
            ?? throw new InvalidOperationException("the field's menu offers no Undo of its own after an address was finished");
        Assert.True(app.Agent.Find(Filter.ById("RecipientRedo")) is null, "the menu offers Redo with nothing undone, as a text box never does");
        Input.Click(app.Steady(undo));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(400));
        Assert.Equal(Address, Value(app, to));
        Keys(app, F10, Input.Shift);
        var redo = app.Agent.Wait(Filter.ById("RecipientRedo") with { OnScreen = true }, TimeSpan.FromSeconds(5))
            ?? throw new InvalidOperationException("the field's menu offers no Redo after an undo");
        Input.Click(app.Steady(redo));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(400));
        Assert.True(Pills(app).Count == 1, "the menu's Redo puts the pill back");
    });

    [OfflineFact]
    public void TheBodyAnswersTheChordsAndItsRightClickMenusUndoAndRedoAreTheSameHistory() => Case(nameof(TheBodyAnswersTheChordsAndItsRightClickMenusUndoAndRedoAreTheSameHistory), (app, _) =>
    {
        ClickBody(app);
        Input.Type("hello");
        Thread.Sleep(400);
        Assert.Contains("hello", BodyText(), StringComparison.Ordinal);
        Keys(app, Z, Input.Control);
        Assert.DoesNotContain("hello", BodyText(), StringComparison.Ordinal);
        Keys(app, Y, Input.Control);
        Assert.Contains("hello", BodyText(), StringComparison.Ordinal);

        ClickBody(app, right: true);
        var menu = BodyMenu();
        Assert.True(menu.Count >= 2, "no right-click menu over the body");
        // WebView2's own Redo is never on, since the editor never runs the engine's undo; one that is
        // on here is the host's, reading the bundle.
        Assert.True(menu[0].Current.IsEnabled, "Undo is on over typing the bundle can take back");
        Assert.False(menu[1].Current.IsEnabled, "Redo is off with nothing undone");
        Click(menu[0].Current.BoundingRectangle);
        Assert.DoesNotContain("hello", BodyText(), StringComparison.Ordinal);
        ClickBody(app, right: true);
        menu = BodyMenu();
        Assert.False(menu[0].Current.IsEnabled, "Undo is off once everything is taken back");
        Assert.True(menu[1].Current.IsEnabled, "Redo is on after the menu's own Undo");
        Click(menu[1].Current.BoundingRectangle);
        Assert.Contains("hello", BodyText(), StringComparison.Ordinal);
    });

    [OfflineFact]
    public void SubjectRedoesOnShiftCtrlZAsWellAsCtrlY() => Case(nameof(SubjectRedoesOnShiftCtrlZAsWellAsCtrlY), (app, _) =>
    {
        var subject = app.Agent.Find(Filter.ById("SubjectBox")) ?? throw new InvalidOperationException("no Subject field");
        TypeInto(app, subject, "hello");
        Keys(app, Z, Input.Control);
        Assert.Equal("", Value(app, subject));
        Keys(app, Z, Input.Control, Input.Shift);
        Assert.Equal("hello", Value(app, subject));
    });
}
