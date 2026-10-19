// A count that moves leaves the folder pane where the reader put it (docs/folder-pane.md).
//
// WHY HERE. Mailcal.Tests pins that the account list is updated in place (AccountListTests), but
// whether the drawn pane holds its scroll offset is WinUI's: the NavigationView is virtualised, a
// subfolder's row opens a layout pass after its parent is drawn, and a pane that takes an account's
// rows down and puts them back redraws its open subfolders late and shifts every row below them.
//
// WHY THE HARNESS. The shift needs open subfolders above the rows the pane shows, and the showcase
// has none. The harness seeds Archive/2025 and Archive/2026 near the top (docker/stalwart/README.md,
// "Nested folders"), and Projects/Clients/Acme holds the message whose read state this moves.
//
// WHY A SMALL WINDOW. Whether the pane scrolls depends on the desktop, so the window's height is
// set, and put back afterwards: the app keeps its bounds for the next launch. The message is marked
// unread and read again, so the suite leaves the mailbox as it found it.

using System.Runtime.InteropServices;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ImapHarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class FolderPanePositionTests(ImapHarnessApp harness)
{
    private const string Subject = "Kickoff notes";
    private static readonly string[] Opened = ["Archive", "Projects", "Clients"];

    private static Element Account(App app) =>
        app.Agent.Find(Filter.Named(Harness.Alice, "ListItem") with { Window = app.Mailbox.Hwnd })
        ?? throw new InvalidOperationException($"the pane has no row for {Harness.Alice}");

    private static Element? FolderRow(App app, string folder) =>
        app.Agent.Find(Filter.Named(folder, "ListItem") with { Under = Account(app) });

    /// <summary>The account's folder rows drawn whole inside <paramref name="viewport"/>, top to
    /// bottom, each as "name@y". A row scrolled past the top reports what is left of it, a shorter
    /// rectangle, so a row counts when it is as tall as rows most often are.</summary>
    private static string ShownRows(App app, Element viewport)
    {
        var rows = app.Agent.FindAll(new Filter { Type = "ListItem", Under = Account(app) })
            .Where(r => r.Rendered && r.Y >= viewport.Y - 1 && r.Y + r.Height <= viewport.Y + viewport.Height + 1)
            .ToList();
        if (rows.Count == 0)
        {
            return "";
        }
        var whole = rows.GroupBy(r => (int)r.Height).OrderByDescending(g => g.Count()).First().Key;
        return string.Join(", ", rows.Where(r => r.Height >= whole - 1).OrderBy(r => r.Y).Select(r => $"{r.Name}@{(int)r.Y}"));
    }

    /// <summary>What Acme's row says about its unread mail: "" when it draws no count, null while
    /// the pane has no Acme row, which a pane rebuilding its rows has for a moment.</summary>
    private static string? AcmeUnread(App app)
    {
        if (FolderRow(app, "Acme") is not { } acme)
        {
            return null;
        }
        var counts = Catalog.Everywhere("a11y_unread_count").Concat(Catalog.Everywhere("a11y_unread_count_one"))
            .Select(pattern => new System.Text.RegularExpressions.Regex("^" + System.Text.RegularExpressions.Regex.Escape(pattern).Replace(@"\{count}", @"\d+") + "$"))
            .ToList();
        return string.Concat(app.Agent.FindAll(new Filter { Under = acme }).Select(e => e.Name).Where(name => counts.Any(c => c.IsMatch(name))));
    }

    /// <summary>Presses the toolbar's read toggle and gives Acme's count ten seconds to read
    /// <paramref name="expect"/>. What it reads is asserted after the rows, so a pane that lost its
    /// rows fails on the rows.</summary>
    private static void SwitchReadState(App app, string expect)
    {
        app.Agent.Invoke(app.Agent.Find(Filter.ById("SelectionMarkRead", "Button")) ?? throw new InvalidOperationException("the toolbar has no read toggle"));
        App.Eventually(() => AcmeUnread(app) == expect, TimeSpan.FromSeconds(10));
    }

    [HarnessFact]
    public void AnUnreadCountThatMovesLeavesTheRowsThePaneShowsWhereTheyWere()
    {
        var oneUnread = Catalog.English("a11y_unread_count_one").Replace("{count}", "1");
        System.Drawing.Rectangle saved = default;
        var maximised = false;
        harness.Case(nameof(AnUnreadCountThatMovesLeavesTheRowsThePaneShowsWhereTheyWere), app =>
        {
            var hwnd = (nint)app.Mailbox.Hwnd;
            saved = Desktop.Bounds(app.Mailbox.Hwnd);
            maximised = IsZoomed(hwnd);
            var scale = Input.Scale(app.Mailbox.Hwnd);
            ShowWindow(hwnd, Restore);
            MoveWindow(hwnd, saved.X, saved.Y, (int)(1200 * scale), (int)(560 * scale), true);
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
            app.Agent.Expand(Account(app));
            foreach (var folder in Opened)
            {
                app.Agent.Expand(app.Agent.Wait(Filter.Named(folder, "ListItem") with { Under = Account(app) }, App.Patience)
                    ?? throw new InvalidOperationException($"{Harness.Alice} shows no {folder} row; was the harness seeded with its nested folders?"));
            }
            app.OpenFolder("Acme");
            app.Agent.Invoke(app.RowTitled($"*{Subject}*"));

            var pane = app.Agent.Find(Filter.ById("MenuItemsScrollViewer") with { Window = app.Mailbox.Hwnd })
                ?? throw new InvalidOperationException("the folder pane has no scroller");
            if (!app.Agent.ScrollTo(pane, 100))
            {
                throw new InvalidOperationException("the pane does not scroll at this window size");
            }
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(600));
            var before = ShownRows(app, pane);
            // Scrolled to its end, so a row it does not show is above the ones it does.
            Assert.True(!before.Contains("2026@", StringComparison.Ordinal) && before.Split(", ").Length > 2,
                $"Archive's open subfolders are above the rows the pane shows (it shows {before})");

            // A pane that rebuilt its rows opens its subfolders again a layout pass later; the shift
            // that leaves is what the reader sees, so it is read once that has had time to land.
            SwitchReadState(app, oneUnread);
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromSeconds(2), cap: TimeSpan.FromSeconds(4));
            Assert.Equal(before, ShownRows(app, pane));
            Assert.Equal(oneUnread, AcmeUnread(app));

            SwitchReadState(app, "");
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(500), floor: TimeSpan.FromSeconds(2), cap: TimeSpan.FromSeconds(4));
            Assert.Equal(before, ShownRows(app, pane));
            Assert.Equal("", AcmeUnread(app));
        }, app =>
        {
            // A case that stopped between the two presses leaves the message unread.
            if (AcmeUnread(app) == oneUnread)
            {
                SwitchReadState(app, "");
            }
            // Innermost first: a shut parent takes its children's rows out of reach.
            foreach (var folder in Opened.Reverse())
            {
                if (FolderRow(app, folder) is { } row)
                {
                    app.Agent.Collapse(row);
                }
            }
            var hwnd = (nint)app.Mailbox.Hwnd;
            MoveWindow(hwnd, saved.X, saved.Y, saved.Width, saved.Height, true);
            if (maximised)
            {
                ShowWindow(hwnd, Maximise);
            }
        });
    }

    private const int Restore = 9;
    private const int Maximise = 3;

    [DllImport("user32.dll")] private static extern bool MoveWindow(nint hwnd, int x, int y, int width, int height, bool repaint);
    [DllImport("user32.dll")] private static extern bool IsZoomed(nint hwnd);
    [DllImport("user32.dll")] private static extern bool ShowWindow(nint hwnd, int command);
}
