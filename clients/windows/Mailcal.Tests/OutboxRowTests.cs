// What an Outbox row says about itself, and which of them offer their actions (docs/sending.md).
//
// Every rule here fails silently in the running app. A state mapped to the wrong word tells
// someone their message is still waiting when it is already on its way; a row with an empty middle
// line reads as a rendering fault rather than a message with no subject; and an actionable
// unconfirmed row offers "Send now" on a message that may already be in front of its recipients,
// which is how it arrives twice. Nothing on screen looks wrong in any of the three.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class OutboxRowTests
{
    private static readonly OutboxLabels Labels = new(
        "Waiting to send",
        "Sending…",
        "Delivery not confirmed",
        "clock",
        "plane",
        "warning",
        "(no subject)");

    private static QueuedRow Queued(
        string account = "acct-1",
        ulong op = 7,
        string to = "you@test.local",
        string subject = "Hi",
        QueuedState state = QueuedState.Waiting) =>
        new(account, op, to, subject, state, Attempts: 1, Detail: null);

    private static List<QueuedRowItem> Build(params QueuedRow[] queued) =>
        OutboxRows.Build(queued, Labels, id => id + "@example.com");

    [Fact]
    public void A_waiting_message_reads_plainly_and_offers_its_actions()
    {
        var row = Assert.Single(Build(Queued()));

        Assert.Equal("you@test.local", row.ToText);
        Assert.Equal("Hi", row.SubjectText);
        // Rule 18 (docs/folder-pane.md): this list holds every account's mail at once, so the row
        // says which one it is waiting on rather than leaving the reader to guess from the pane.
        Assert.Equal("acct-1@example.com", row.AccountText);
        Assert.Equal(QueuedSendState.Waiting, row.State);
        Assert.Equal("Waiting to send", row.StateText);
        Assert.Equal("clock", row.StateGlyph);
        Assert.True(row.IsActionable);
    }

    [Fact]
    public void A_message_on_its_way_offers_nothing()
    {
        // It cannot be called back, so the row says where it has got to and nothing else.
        var row = Assert.Single(Build(Queued(state: QueuedState.Sending)));

        Assert.Equal(QueuedSendState.Sending, row.State);
        Assert.Equal("Sending…", row.StateText);
        Assert.False(row.IsActionable);
    }

    [Fact]
    public void A_message_whose_delivery_is_unconfirmed_offers_nothing_either()
    {
        // The one that matters: it may already have arrived, so "Send now" is the wrong offer and
        // the warning glyph is the right one. Nothing will retry it by itself.
        var row = Assert.Single(Build(Queued(state: QueuedState.Unconfirmed)));

        Assert.Equal(QueuedSendState.Unconfirmed, row.State);
        Assert.Equal("Delivery not confirmed", row.StateText);
        Assert.Equal("warning", row.StateGlyph);
        Assert.False(row.IsActionable);
    }

    [Fact]
    public void A_message_with_no_subject_says_so_in_the_words_the_rest_of_the_app_uses()
    {
        // A blank subject is ordinary mail, not an error, and a row with an empty middle line
        // reads as one. It is the same absence the message list and the reading header name.
        var row = Assert.Single(Build(Queued(subject: string.Empty)));

        Assert.Equal("(no subject)", row.SubjectText);
        Assert.Equal("you@test.local", row.ToText);
    }

    [Fact]
    public void A_message_addressed_by_bcc_alone_still_has_a_first_line()
    {
        // The core's `to` is the draft's To field, which a Bcc-only message legitimately leaves
        // empty. Left blank the row would read as a rendering fault; the account it goes from is
        // the only thing then known about where it is going.
        var row = Assert.Single(Build(Queued(to: string.Empty)));

        Assert.Equal("acct-1@example.com", row.ToText);
    }

    [Fact]
    public void A_rows_identity_carries_its_account_as_well_as_its_op()
    {
        // An op id is unique only within its own account's queue, and this list holds every
        // account's at once. Keyed on the op alone, two accounts' first queued sends would
        // reconcile as one row and one of the two messages would simply not be on screen.
        var rows = Build(Queued(account: "acct-1", op: 1), Queued(account: "acct-2", op: 1));

        Assert.Equal(2, rows.Count);
        Assert.NotEqual(rows[0].Id, rows[1].Id);
    }

    [Fact]
    public void The_order_the_core_gave_is_the_order_they_go_out_in()
    {
        var rows = Build(Queued(op: 1, subject: "First"), Queued(op: 2, subject: "Second"));

        Assert.Equal(["First", "Second"], rows.Select(r => r.SubjectText));
    }
}
