// What a search puts on screen here, which docs/search.md rules 9 and 10 decide for every client.
// The core half is covered by crates/mailcal-app/src/tests_search/; what it cannot see is whether
// this client draws the row kind the core sent, and whether the field asks the core once or once per
// key: one needs a drawn list, the other a real keystroke.
//
// THE FIXTURE is the en showcase seed, whose one three-message conversation makes these assertions
// rather than coincidences. "no-go" is its subject, so all three match; "rollback" is in the older
// two only, so the row has to be labelled by Eva's reply rather than Tom's later follow-up, which is
// how the mailbox labels the same thread. If a case stops finding its row, the seed changed.

using System.Globalization;
using System.Text.RegularExpressions;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
public sealed partial class SearchTests(ShowcaseApp showcase)
{
    private const string WholeThread = "no-go";
    private const string OlderOnly = "rollback"; // "Q3 launch" also finds Eva's "Q3 board update"
    private const string ThreadTitle = "Re: Q3 launch*";
    private const int ThreadMessages = 3;
    private const string Newest = "Tom de Vries";
    private const string NewestMatch = "Eva Jansen";
    private const string NoMatch = "zzzznothingmatchesthis";

    // `2026-09-12 12:40:39.735 +02:00 [info] [mailcal_app::snapshot] rebuild_snapshot: 1 row(s) of 1 in 5ms`
    [GeneratedRegex(@"(?m)^(?<ts>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3} [+-]\d{2}:\d{2}) .*rebuild_snapshot: ")]
    private static partial Regex Rebuild();

    [GeneratedRegex(@"^\d+ (conversation|message|result)")]
    private static partial Regex CountLine();

    // Each case leaves the list unsearched: a search adds a horizon line under the header, which the
    // next suite on this launch does not expect.
    private void Case(string name, Action<App> body) => showcase.Case(name, body, Clear);

    private static void Clear(App app)
    {
        app.Agent.SetValue(Box(app), "");
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromMilliseconds(500), cap: TimeSpan.FromSeconds(5));
    }

    /// <summary>The search field, the AutoSuggestBox's inner text box.</summary>
    private static Element Box(App app) =>
        app.Agent.Find(Filter.ById("TextBox", "Edit") with { Window = app.Mailbox.Hwnd }) ?? throw new InvalidOperationException("no search field");

    /// <summary>Puts <paramref name="query"/> in the field and waits for the list to hold
    /// <paramref name="expected"/> rows: a search is asynchronous, and the debounce under test
    /// adds a quarter of a second before the core is asked.</summary>
    private static IReadOnlyList<Element> Search(App app, string query, int expected)
    {
        app.Agent.SetValue(Box(app), query);
        IReadOnlyList<Element> rows = [];
        App.Eventually(() => (rows = app.Rows()).Count == expected, TimeSpan.FromSeconds(30));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(300));
        return app.Rows();
    }

    /// <summary>The list's count line ("1 conversation"), which carries no id: the sentence is its
    /// whole job, so reading anything else would not be reading what the user reads.</summary>
    private static string CountFooter(App app) =>
        app.Agent.FindAll(new Filter { Type = "Text", Window = app.Mailbox.Hwnd }).Select(t => t.Name).FirstOrDefault(n => CountLine().IsMatch(n))
        ?? throw new InvalidOperationException("the message list draws no count line");

    [ShowcaseFact]
    public void ASearchListsTheConversationItsMatchesBelongToNotTheMatches() => Case(nameof(ASearchListsTheConversationItsMatchesBelongToNotTheMatches), app =>
        // Rule 9: a flat row per matching message made one thread one thing in the mailbox and
        // another here, and archiving the row did two things on two screens.
        Assert.Single(Search(app, WholeThread, 1)));

    [ShowcaseFact]
    public void AndTheRowCarriesTheWholeConversationNotThePartThatMatched() => Case(nameof(AndTheRowCarriesTheWholeConversationNotThePartThatMatched), app =>
    {
        // The badge is what the row stands for: fewer than the thread's messages is a fragment
        // dressed as a conversation, and open, select and archive would not mean what they mean in
        // the mailbox.
        Search(app, WholeThread, 1);
        var badge = app.Agent.FindAll(new Filter { Type = "Text", Under = app.RowTitled(ThreadTitle) }).FirstOrDefault(t => t.Name.Length > 0 && t.Name.All(char.IsDigit));
        Assert.Equal(ThreadMessages, badge is null ? 0 : int.Parse(badge.Name, CultureInfo.InvariantCulture));
    });

    [ShowcaseFact]
    public void ACountOfOneReadsAsOne() => Case(nameof(ACountOfOneReadsAsOne), app =>
    {
        // The catalog's plural forms from the client's side; nothing else on Windows exercises the
        // singular branch.
        Search(app, WholeThread, 1);
        Assert.Equal("1 conversation", CountFooter(app));
    });

    [ShowcaseFact]
    public void ACountOfNoneStaysPluralWhichIsWhatMakesTheLineAboveAnAssertion() => Case(nameof(ACountOfNoneStaysPluralWhichIsWhatMakesTheLineAboveAnAssertion), app =>
    {
        // English takes the plural at zero; without this, a client saying "1 conversation" for every
        // count passes the case above.
        Search(app, NoMatch, 0);
        Assert.Equal("0 conversations", CountFooter(app));
    });

    [ShowcaseFact]
    public void AConversationIsLabelledByTheMessageThatMatchedNotByItsNewest() => Case(nameof(AConversationIsLabelledByTheMessageThatMatchedNotByItsNewest), app =>
    {
        // The query matches the two older messages, so the row reads as Eva's reply. Labelled by
        // the unrelated later message, it answers nothing the user asked, and sorts the thread by a
        // date no match happened on (rule 1).
        var rows = Search(app, OlderOnly, 1);
        Assert.Single(rows);
        Assert.Contains(NewestMatch, rows[0].Name, StringComparison.Ordinal);
        Assert.DoesNotContain(Newest, rows[0].Name, StringComparison.Ordinal);
    });

    [ShowcaseFact]
    public void TheCoreIsAskedAfterTheTypingStopsNotWhileItIsHappening() => Case(nameof(TheCoreIsAskedAfterTheTypingStopsNotWhileItIsHappening), app =>
    {
        // Rule 10's client half: only the client knows a keystroke happened, so only it can stop nine
        // searches being started to type one word. What is measured is the gap between one key and
        // the rebuild that answers it, never a count of rebuilds, which depends on how keys race the
        // event pump. The key timed is sent into a quiet core: timed off the last key of a word, this
        // read whichever rebuild came next, often the prefix's, and went red on a loaded machine.
        Clear(app);
        app.ClickToType(Box(app));
        foreach (var character in "quarterly")
        {
            Input.Type(character.ToString());
            Thread.Sleep(120);
        }
        Assert.True(WaitCoreQuiet(), "the searches the typing asked for never finished, so the key below would be timed against one of them");
        var before = AppLog.Length();
        Input.Type("x");
        // Stamped at once: anything between the key and this line is subtracted from the interval.
        var lastKey = DateTimeOffset.Now;
        Thread.Sleep(3000);
        Assert.Equal("quarterlyx", app.Agent.Read(Box(app)).Value);
        var answered = Rebuild().Matches(AppLog.Since(before))
            .Select(m => DateTimeOffset.Parse(m.Groups["ts"].Value, CultureInfo.InvariantCulture))
            .Where(at => at >= lastKey)
            .Cast<DateTimeOffset?>()
            .FirstOrDefault()
            ?? throw new InvalidOperationException("the core rebuilt nothing after the last key, so the key never reached it");
        var waited = (answered - lastKey).TotalMilliseconds;
        Assert.True(waited >= 200,
            $"the list was rebuilt {waited:0}ms after the last key; a search is a full-text query per account plus a store read per hit, so the field goes quiet for 250ms first (docs/search.md, \"Typing does not mean searching\")");
    });

    /// <summary>Waits until the log has stopped growing for a second: the core writes a line per
    /// rebuild, so a quiet log is a core that has stopped answering.</summary>
    private static bool WaitCoreQuiet()
    {
        var last = AppLog.Length();
        var quietSince = DateTime.UtcNow;
        return App.Eventually(() =>
        {
            Thread.Sleep(150);
            var now = AppLog.Length();
            if (now != last)
            {
                last = now;
                quietSince = DateTime.UtcNow;
            }
            return DateTime.UtcNow - quietSince >= TimeSpan.FromSeconds(1);
        }, TimeSpan.FromSeconds(8));
    }
}
