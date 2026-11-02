// Where the reading pane goes after an archive from it (docs/reading-actions.md, "Where the pane
// goes next"): the message the core names, opened as clicking its row would.
//
// WHY HERE. Which message that is is the core's, pinned in reading_stop_tests.rs; what only this can
// see is that the client asks it, through the C# binding, and opens the answer where a click would
// have. The fixture is a conversation whose newest message is the owner's reply filed in Sent,
// below a message the case archives: the Inbox row summarises the received message, so a client
// deciding by the row alone opens Bob's message, and one asking the core opens Alice's reply.
//
// WHY THE HARNESS. An archive is a mail write. The case imports its own three messages and takes
// them off the server again, so the seed is left as it was.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed class ReadingPaneAdvanceTests(HarnessApp harness)
{
    private static readonly string Inbox = Catalog.English("folder_inbox");

    private static string Message(string from, string to, string subject, string id, string? replyTo = null) =>
        string.Join("\r\n", new[]
        {
            $"From: {from}", $"To: {to}", $"Subject: {subject}", $"Date: {DateTime.UtcNow:r}",
            $"Message-ID: <{id}@test.local>",
        }
        .Concat(replyTo is null ? [] : new[] { $"In-Reply-To: <{replyTo}@test.local>", $"References: <{replyTo}@test.local>" })
        .Concat(new[] { "MIME-Version: 1.0", "Content-Type: text/plain; charset=utf-8", "", "Pane fixture.", "" }));

    /// <summary>Imports one message into <paramref name="role"/>, a second after the last: a
    /// conversation is ordered by when its messages arrived (docs/stalwart README).</summary>
    private static void Import(string message, string role, params string[] keywords)
    {
        Thread.Sleep(1100);
        Harness.ImportAsync(message, role, keywords).GetAwaiter().GetResult();
    }

    [HarnessFact]
    public void ArchivingMovesThePaneOnToTheNewestMessageOfTheConversationBelow()
    {
        var run = DateTime.Now.ToString("HHmmss", System.Globalization.CultureInfo.InvariantCulture);
        var conversation = $"Pane conversation {run}";
        var archived = $"Pane archived {run}";
        harness.Case(nameof(ArchivingMovesThePaneOnToTheNewestMessageOfTheConversationBelow), app =>
        {
            // Bob writes, Alice answers from Sent, then the message to archive lands above both.
            Import(Message($"Bob Fixture <{Harness.Bob}>", Harness.Alice, conversation, $"pane-x-{run}"), "inbox");
            Import(Message($"Alice Fixture <{Harness.Alice}>", Harness.Bob, $"Re: {conversation}", $"pane-y-{run}", replyTo: $"pane-x-{run}"), "sent");
            Import(Message($"Bob Fixture <{Harness.Bob}>", Harness.Alice, archived, $"pane-m-{run}"), "inbox");

            app.OpenFolder(Inbox);
            var sync = app.Agent.Find(Filter.ById("SelectionSync", "Button"))!;
            app.Agent.Invoke(sync);
            if (!App.Eventually(() => Listed(app, archived) && Listed(app, conversation), TimeSpan.FromSeconds(30)))
            {
                throw new InvalidOperationException("the imported messages never reached the list");
            }
            var below = app.RowTitled(conversation);
            Assert.True(app.Agent.FindAll(new Filter { Type = "Text", Under = below }).Any(t => t.Name == "2"),
                $"'{conversation}' is a conversation of two, the reply in Sent included; did the harness thread it?");

            app.OpenMessage(archived);
            app.Agent.Invoke(app.Agent.Wait(Filter.ById("ReadingArchive", "Button"), TimeSpan.FromSeconds(10))
                ?? throw new InvalidOperationException("the reading pane offers no Archive"));

            Assert.True(App.Eventually(() => ReadingFrom(app, "Alice Fixture"), TimeSpan.FromSeconds(20)),
                "the pane did not move on to Alice's reply, the conversation's newest message; a pane deciding by the row opens Bob's");
            Assert.True(app.Agent.FindAll(new Filter { Type = "Button", Under = app.RowTitled(conversation) }).Count(b => b.Rendered) > 1,
                "the pane opened a message of a collapsed conversation without expanding it, as a click on its row would");
        },
        _ =>
        {
            Harness.RemoveMail(Harness.Alice, conversation);
            Harness.RemoveMail(Harness.Alice, archived);
        });
    }

    /// <summary>
    /// The answer is a draft when the conversation below's newest message is one, and the pane
    /// opens it as a click would: resumed in a composer wherever it is listed (docs/drafts.md), here
    /// in the Inbox, never read-only in the pane.
    /// </summary>
    [HarnessFact]
    public void ArchivingMovesThePaneOnToADraftItsComposerOpens()
    {
        var run = DateTime.Now.ToString("HHmmss", System.Globalization.CultureInfo.InvariantCulture);
        var conversation = $"Pane draft {run}";
        var archived = $"Pane before draft {run}";
        harness.Case(nameof(ArchivingMovesThePaneOnToADraftItsComposerOpens), app =>
        {
            // Bob writes, Alice starts an answer she has not sent, then the message to archive lands
            // above both.
            Import(Message($"Bob Fixture <{Harness.Bob}>", Harness.Alice, conversation, $"pane-dx-{run}"), "inbox");
            Import(Message($"Alice Fixture <{Harness.Alice}>", Harness.Bob, $"Re: {conversation}", $"pane-dy-{run}", replyTo: $"pane-dx-{run}"), "drafts", "$draft", "$seen");
            Import(Message($"Bob Fixture <{Harness.Bob}>", Harness.Alice, archived, $"pane-dm-{run}"), "inbox");

            app.OpenFolder(Inbox);
            app.Agent.Invoke(app.Agent.Find(Filter.ById("SelectionSync", "Button"))!);
            if (!App.Eventually(() => Listed(app, archived) && Listed(app, conversation), TimeSpan.FromSeconds(30)))
            {
                throw new InvalidOperationException("the imported messages never reached the list");
            }

            app.OpenMessage(archived);
            app.Agent.Invoke(app.Agent.Wait(Filter.ById("ReadingArchive", "Button"), TimeSpan.FromSeconds(10))
                ?? throw new InvalidOperationException("the reading pane offers no Archive"));

            Assert.True(app.Agent.Wait(Filter.ById("SendButton"), App.Patience) is not null,
                "the pane moved on to a draft without resuming it in a composer, as a click on its row would");
            var subject = app.Agent.Find(Filter.ById("SubjectBox"));
            Assert.True(subject is not null && app.Agent.Read(subject).Value == $"Re: {conversation}",
                "the composer the pane opened does not hold the draft it moved on to");
        },
        app =>
        {
            // Discard takes the fixture's draft off the server with the composer; the rest goes by subject.
            app.CloseComposer();
            Harness.RemoveMail(Harness.Alice, conversation);
            Harness.RemoveMail(Harness.Alice, archived);
        });
    }

    private static bool Listed(App app, string subject)
    {
        try
        {
            app.RowTitled(subject);
            return true;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
    }

    /// <summary>Whether the reading pane, right of the list, names <paramref name="sender"/>.</summary>
    private static bool ReadingFrom(App app, string sender)
    {
        var list = app.Agent.Read(app.RowsList()).Element;
        return app.Agent.FindAll(new Filter { Type = "Text", NameLike = $"*{sender}*", Window = app.Mailbox.Hwnd })
            .Any(text => text.Rendered && text.X >= list.X + list.Width);
    }
}
