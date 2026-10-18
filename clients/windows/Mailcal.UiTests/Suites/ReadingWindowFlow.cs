// The moves the reading-window suite makes: which message the pane is showing.

using Allodia.Mailcal.UiTests.Driver;

namespace Allodia.Mailcal.UiTests.Suites;

internal sealed class ReadingWindowFlow(App app)
{
    /// <summary>Seeded, with an invitation card, so its window has content to draw.</summary>
    internal const string RowSubject = "Quarterly planning";

    internal const string OtherSubject = "HTML message with a remote image";

    /// <summary>The seeded two-message conversation, by a pattern: the row is titled by whichever
    /// message is newest, the reply on a freshly seeded harness and the original after a re-sync.</summary>
    internal const string ThreadSubject = "*Project kickoff";

    /// <summary>The action row in the order docs/reading-actions.md fixes, overflow last of all.</summary>
    internal static readonly string[] RowButtons =
        ["ReadingReply", "ReadingReplyAll", "ReadingForward", "ReadingArchive", "ReadingDelete", "ReadingOverflow"];

    /// <summary>The subject the reading pane shows, or "" on its placeholder. The mailbox's own: the
    /// agent reads each window apart, so a reading window's subject cannot answer for the pane's.</summary>
    internal string PaneSubject() =>
        app.Agent.Find(Filter.ById("ReadingSubject") with { Window = app.Mailbox.Hwnd })?.Name ?? "";
}
