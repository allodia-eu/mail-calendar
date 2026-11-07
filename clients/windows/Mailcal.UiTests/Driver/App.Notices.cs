// The window's notice bar (docs/notices.md): finding it, what it says, and an account row's mark.

namespace Allodia.Mailcal.UiTests.Driver;

internal sealed partial class App
{
    /// <summary>A notice's sentence in English, its account named by <paramref name="address"/>.</summary>
    internal static string NoticeWords(string key, string? address = null) =>
        address is null ? Catalog.English(key) : Catalog.English(key).Replace("{address}", address);

    /// <summary>The notice bar while it is drawn, or null.</summary>
    internal Element? NoticeBar() =>
        Agent.Find(Filter.ById("NoticeBar") with { OnScreen = true, Window = Mailbox.Hwnd });

    /// <summary>Whether <paramref name="under"/> or anything in it is named <paramref name="text"/>.</summary>
    internal bool Says(Element under, string text) =>
        under.Name == text || Agent.FindAll(new Filter { Under = under }).Any(e => e.Name == text);

    /// <summary>Waits for the bar to draw <paramref name="sentence"/>, and returns it.</summary>
    internal Element? WaitNotice(string sentence, TimeSpan timeout)
    {
        Element? found = null;
        Eventually(() => (found = NoticeBar()) is { } bar && Says(bar, sentence), timeout);
        return found is { } bar && Says(bar, sentence) ? bar : null;
    }

    /// <summary>The bar's action button, or null when its notice has none.</summary>
    internal Element? NoticeAction(Element bar) =>
        Agent.Find(new Filter { Id = "NoticeAction", Type = "Button", Under = bar, OnScreen = true });

    /// <summary>The bar's close button, or null when its notice cannot be closed. The InfoBar's own,
    /// found by the name its template gives it.</summary>
    internal Element? NoticeClose(Element bar) =>
        Agent.Find(new Filter { Id = "CloseButton", Type = "Button", Under = bar, OnScreen = true });

    /// <summary>The label <paramref name="address"/>'s row in the folder pane carries for its
    /// notices, or null when it carries none.</summary>
    internal string? AccountMark(string address)
    {
        var row = Agent.Find(Filter.Named(address, "ListItem") with { Window = Mailbox.Hwnd });
        var label = row is null ? null : Agent.Find(new Filter { Id = "AccountNoticeLabel", Under = row, OnScreen = true });
        return label?.Name;
    }
}
