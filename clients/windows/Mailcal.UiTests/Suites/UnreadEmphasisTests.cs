// Unread mail looks unread, the subject and the sender both, on a flat row and on a conversation
// header alike (the Android twin is UnreadEmphasisTest.kt).
//
// The suite that would have caught the bug it was written for: the conversation header bolded
// nothing on unread mail, not because the binding was wrong but because MailRow.Unread was never
// assigned for a thread row. Nothing without a drawn window sees that. The weight is read exactly,
// off the text as drawn: 400 is Normal, 600 SemiBold.
//
// THE FIXTURE is the en showcase seed (crates/mailcal-bindings/src/showcase_data/en.rs). The
// harness cannot stand in: every message seeded there is read. If a case stops finding its row,
// that seed changed; check the message is still unread (no `.seen()`) before assuming the app broke.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class UnreadEmphasisTests(ShowcaseApp showcase)
{
    private static readonly (string Title, string Sender) Unread = ("Welcome to Allodia Mail*", "Allodia");
    private static readonly (string Title, string Sender) Read = ("Your June usage report", "Example Cloud");

    // Three messages, two of them unread: the conversation the projection could never bold.
    private static readonly (string Title, string Sender) UnreadThread = ("Re: Q3 launch*", "Tom de Vries");

    private const int SemiBold = 600;
    private const int Normal = 400;

    /// <summary>The weight of the text in <paramref name="title"/>'s row matching
    /// <paramref name="text"/>. Scoped to the row: the reading pane can draw the same words.</summary>
    private static int Weight(App app, string title, string text)
    {
        var label = app.Agent.Find(new Filter { Type = "Text", Under = app.RowTitled(title), NameLike = text })
            ?? throw new InvalidOperationException($"the row '{title}' draws no text matching '{text}'");
        return app.Agent.Read(label).FontWeight ?? throw new InvalidOperationException($"'{label.Name}' reports no font weight");
    }

    [ShowcaseFact]
    public void AnUnreadMessageBoldsItsSubject() => showcase.Case(nameof(AnUnreadMessageBoldsItsSubject), app =>
        Assert.Equal(SemiBold, Weight(app, Unread.Title, Unread.Title)));

    [ShowcaseFact]
    public void AnUnreadMessageBoldsItsSenderToo() => showcase.Case(nameof(AnUnreadMessageBoldsItsSenderToo), app =>
        // The accent dot alone is a small target for the eye, and the sender is what says whether
        // an unread row is worth opening.
        Assert.Equal(SemiBold, Weight(app, Unread.Title, Unread.Sender)));

    [ShowcaseFact]
    public void AReadMessageLeavesSubjectAndSenderAlone() => showcase.Case(nameof(AReadMessageLeavesSubjectAndSenderAlone), app =>
    {
        Assert.Equal(Normal, Weight(app, Read.Title, Read.Title));
        // So bolding unread rows cannot make the read ones louder too.
        Assert.Equal(Normal, Weight(app, Read.Title, Read.Sender));
    });

    [ShowcaseFact]
    public void UnreadAndReadAreActuallyDistinguishable() => showcase.Case(nameof(UnreadAndReadAreActuallyDistinguishable), app =>
    {
        // One weight returned from both arms compiles, renders and passes every case above against
        // a pair of constants that happen to match. This one cannot.
        Assert.True(Weight(app, Unread.Title, Unread.Title) > Weight(app, Read.Title, Read.Title), "an unread subject is heavier than a read one");
        Assert.True(Weight(app, Unread.Title, Unread.Sender) > Weight(app, Read.Title, Read.Sender), "and so is an unread sender");
    });

    [ShowcaseFact]
    public void AnUnreadConversationHeaderBoldsSubjectAndSender() => showcase.Case(nameof(AnUnreadConversationHeaderBoldsSubjectAndSender), app =>
    {
        // A conversation is unread while anything in it is: the header may not read as settled
        // while it hides an unread reply.
        Assert.Equal(SemiBold, Weight(app, UnreadThread.Title, UnreadThread.Title));
        Assert.Equal(SemiBold, Weight(app, UnreadThread.Title, UnreadThread.Sender));
    });
}
