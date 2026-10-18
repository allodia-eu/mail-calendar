// The moves the Drafts suite makes: writing and storing a draft, leaving its composer, finding it in
// the account's Drafts folder as the server lists it, and taking it off the server again.

using Allodia.Mailcal.UiTests.Driver;

namespace Allodia.Mailcal.UiTests.Suites;

internal sealed class DraftsFlow(App app)
{
    /// <summary>Per run, so a copy left behind by an interrupted run is never mistaken for this run's.</summary>
    internal static readonly string Run = DateTime.Now.ToString("HHmmss", System.Globalization.CultureInfo.InvariantCulture);

    internal static string Saved => Catalog.English("compose_draft_saved");

    internal static string Reply => Catalog.English("action_reply");

    private Agent Agent => app.Agent;

    /// <summary>
    /// Opens Drafts, asks the server for it, and waits until <paramref name="subject"/> is listed (or,
    /// with <paramref name="absent"/>, is not). The sync matters for the absent case: without it a
    /// removal the list has not heard of yet reads as the row still being there.
    /// </summary>
    internal bool WaitDraftRow(string subject, bool absent = false, int timeoutSec = 30)
    {
        app.OpenFolder("Drafts");
        if (Agent.Find(Filter.ById("SelectionSync", "Button")) is { } sync)
        {
            Agent.Invoke(sync);
        }
        var row = new Filter { Type = "ListItem", Under = app.RowsList(), Has = new Filter { NameLike = subject } };
        var timeout = TimeSpan.FromSeconds(timeoutSec);
        return absent ? Agent.WaitGone(row, timeout) : Agent.Wait(row, timeout) is not null;
    }

    /// <summary>The composer's draft hint, or "" when it draws none.</summary>
    internal string DraftHint() => Agent.Find(Filter.ById("DraftHint"))?.Name ?? "";

    internal bool WaitDraftSaved(int timeoutSec = 15) =>
        Agent.Wait(Filter.ById("DraftHint") with { Name = Saved }, TimeSpan.FromSeconds(timeoutSec)) is not null;

    internal void OpenNewComposer()
    {
        var compose = Agent.Wait(Filter.ById("ComposeButton"), App.Patience)
            ?? throw new InvalidOperationException("no ComposeButton within 30 s, the mail list never drew its action row");
        Agent.Invoke(compose);
        if (Agent.Wait(Filter.ById("SendButton"), App.Patience) is null)
        {
            throw new InvalidOperationException("no composer within 30 s of pressing Compose");
        }
        WaitEditorSeeded();
    }

    /// <summary>Waits for the composer's editor to finish seeding: the app's own word for it, since
    /// a page still loading looks no different on screen.</summary>
    internal void WaitEditorSeeded()
    {
        if (Agent.Wait(Filter.ById("Editor") with { Signal = "seeded" }, App.Patience) is null)
        {
            throw new InvalidOperationException("the composer's editor did not finish seeding within 30 s");
        }
    }

    /// <summary>Types into a field the way UI Automation does, which the field takes as an edit.</summary>
    internal void SetText(string id, string text)
    {
        var field = Agent.Find(Filter.ById(id)) ?? throw new InvalidOperationException($"the composer has no #{id}");
        Agent.SetValue(field, text);
        Agent.Settle();
    }

    /// <summary>Writes a new message addressed and titled, and stores it with Save as draft. Leaves the
    /// composer open.</summary>
    internal void NewSavedDraft(string subject)
    {
        OpenNewComposer();
        SetText("ToField", Harness.Bob);
        SetText("SubjectBox", subject);
        var save = Agent.Find(Filter.ById("SaveDraftButton", "Button"))
            ?? throw new InvalidOperationException("the composer draws no Save as draft button");
        Agent.Invoke(save);
        if (!WaitDraftSaved())
        {
            throw new InvalidOperationException($"Save as draft never reported the draft saved; the hint reads '{DraftHint()}'");
        }
    }

    /// <summary>Whether "Discard draft?" is up: the only dialog these cases raise, so its primary
    /// button is it. It arrives after the press returns, hence the floor.</summary>
    internal bool DiscardQuestionUp()
    {
        Agent.Settle(floor: TimeSpan.FromMilliseconds(300), cap: TimeSpan.FromMilliseconds(800));
        return Agent.Find(Filter.ById("PrimaryButton", "Button")) is not null;
    }

    /// <summary>Leaves the open composer the way a person does, by opening a message from the Inbox.
    /// Leaving keeps the draft and asks nothing (docs/drafts.md), so a question here is a failure.</summary>
    internal void CloseDraftComposer()
    {
        if (Agent.Find(Filter.ById("SendButton")) is null)
        {
            return;
        }
        app.OpenFolder("Inbox");
        Agent.Invoke(app.Rows()[0]);
        if (DiscardQuestionUp())
        {
            throw new InvalidOperationException("leaving the composer asked \"Discard draft?\"; leaving keeps the draft unasked");
        }
        Agent.WaitGone(Filter.ById("SendButton"), TimeSpan.FromSeconds(10));
    }

    /// <summary>Opens the Drafts row titled <paramref name="subject"/> and waits for its composer.</summary>
    internal void OpenStoredDraft(string subject)
    {
        if (!WaitDraftRow(subject))
        {
            throw new InvalidOperationException($"Drafts does not list '{subject}'");
        }
        Agent.Invoke(app.RowTitled(subject));
        if (Agent.Wait(Filter.ById("SendButton"), App.Patience) is null)
        {
            throw new InvalidOperationException($"the Drafts row '{subject}' opened no composer within 30 s");
        }
        WaitEditorSeeded();
    }

    /// <summary>
    /// Takes <paramref name="subject"/> off the server however a case left it, so the next run starts
    /// from the same Drafts. Discarded rather than left: leaving would store whatever a failed case
    /// typed. Asked as "is it gone" rather than "is it there": right after a discard the list still
    /// shows the row until its sync lands, and reopening a draft the server no longer has fails.
    /// </summary>
    internal void ClearStoredDraft(string subject)
    {
        app.CloseExtraWindows();
        app.CloseComposer();
        if (WaitDraftRow(subject, absent: true, timeoutSec: 5))
        {
            return;
        }
        OpenStoredDraft(subject);
        app.CloseComposer();
    }

    /// <summary>Stores a draft as another client would, straight into Drafts over JMAP: an HTML body
    /// whose only picture is a <c>cid:</c> part, a solid red square, ahead of one line of text.</summary>
    internal static Task AddForeignDraft(string subject) => Harness.ImportAsync(string.Join("\r\n",
        $"From: {Harness.Alice}", $"To: {Harness.Bob}", $"Subject: {subject}",
        $"Date: {DateTime.UtcNow:r}", $"Message-ID: <{Guid.NewGuid()}@test.local>",
        "MIME-Version: 1.0", "Content-Type: multipart/related; boundary=\"part\"", "",
        "--part", "Content-Type: text/html; charset=utf-8", "",
        "<p><img src=\"cid:square@test.local\" alt=\"\" width=\"40\" height=\"40\"></p><p>Written elsewhere</p>",
        "--part", "Content-Type: image/png", "Content-ID: <square@test.local>", "Content-Transfer-Encoding: base64", "",
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==",
        "--part--", ""), "drafts", "$draft", "$seen");

    /// <summary>How many pixels inside the open composer's editor are the fixture's red, sampled
    /// every second pixel each way.</summary>
    internal int EditorRedPixels()
    {
        var editor = Agent.Find(Filter.ById("Editor")) ?? throw new InvalidOperationException("the composer has no editor");
        if (!editor.Rendered)
        {
            throw new InvalidOperationException("the message editor is in the tree but not drawn");
        }
        using var pixels = App.Pixels(editor);
        var red = 0;
        for (var x = 0; x < pixels.Width; x += 2)
        {
            for (var y = 0; y < pixels.Height; y += 2)
            {
                var c = pixels.GetPixel(x, y);
                if (c.R > 200 && c.G < 60 && c.B < 60)
                {
                    red++;
                }
            }
        }
        return red;
    }
}
