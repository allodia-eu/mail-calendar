// What an Outbox row says about itself, what its menu offers, and which intent each item sends
// (docs/sending.md).
//
// Every rule here fails silently in the running app. A state mapped to the wrong word tells
// someone their message is still waiting when it is already on its way; a row with an empty middle
// line reads as a rendering fault rather than a message with no subject; and an unconfirmed row
// offering "Send now", or answering "Send again" with the wrong intent, sends a message that may
// already be in front of its recipients, which is how it arrives twice. Nothing on screen looks
// wrong in any of them.

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
        "Not sent",
        "clock",
        "plane",
        "warning",
        "error",
        "(no subject)");

    private static QueuedRow Queued(
        string account = "acct-1",
        ulong op = 7,
        string to = "you@test.local",
        string subject = "Hi",
        QueuedState state = QueuedState.Waiting,
        bool editable = true) =>
        new(account, op, to, subject, state, Attempts: 1, Detail: null, Editable: editable);

    private const string Staging = "/tmp/resumed-drafts/abc";

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
        Assert.Equal([QueuedAction.SendNow, QueuedAction.Edit, QueuedAction.Cancel], row.Actions);
        Assert.True(row.IsActionable);
    }

    [Fact]
    public void A_message_on_its_way_lists_the_waiting_items_disabled()
    {
        // It cannot be called back, so nothing can be chosen; the menu keeps the shape it had a
        // moment ago rather than opening empty.
        var row = Assert.Single(Build(Queued(state: QueuedState.Sending)));

        Assert.Equal(QueuedSendState.Sending, row.State);
        Assert.Equal("Sending…", row.StateText);
        Assert.Equal("plane", row.StateGlyph);
        Assert.Equal([QueuedAction.SendNow, QueuedAction.Edit, QueuedAction.Cancel], row.Actions);
        Assert.False(row.IsActionable);
    }

    [Fact]
    public void A_message_whose_delivery_is_unconfirmed_offers_only_the_two_answers()
    {
        // The one that matters: it may already have arrived, so Send now, Edit and Cancel are the
        // wrong offers and the warning glyph is the right one. Nothing will retry it by itself.
        var row = Assert.Single(Build(Queued(state: QueuedState.Unconfirmed)));

        Assert.Equal(QueuedSendState.Unconfirmed, row.State);
        Assert.Equal("Delivery not confirmed", row.StateText);
        Assert.Equal("warning", row.StateGlyph);
        Assert.Equal([QueuedAction.MarkSent, QueuedAction.ConfirmNotSent], row.Actions);
        Assert.DoesNotContain(QueuedAction.SendNow, row.Actions);
        Assert.DoesNotContain(QueuedAction.Edit, row.Actions);
        Assert.DoesNotContain(QueuedAction.Cancel, row.Actions);
        Assert.True(row.IsActionable);
    }

    [Fact]
    public void A_refused_message_offers_to_send_again_edit_or_discard()
    {
        var row = Assert.Single(Build(Queued(state: QueuedState.NotSent)));

        Assert.Equal(QueuedSendState.NotSent, row.State);
        Assert.Equal("Not sent", row.StateText);
        Assert.Equal("error", row.StateGlyph);
        Assert.Equal([QueuedAction.SendAgain, QueuedAction.Edit, QueuedAction.Discard], row.Actions);
        Assert.True(row.IsActionable);
    }

    [Fact]
    public void A_message_no_composer_can_hold_never_offers_edit()
    {
        // An invitation answer: the core would refuse the edit, so the item would do nothing. The
        // rest of the row's menu keeps its order, and a row in flight keeps the shape it had.
        // A loop rather than a theory: the generated QueuedState is internal, and a theory's
        // parameter must be as visible as the test.
        foreach (var state in new[] { QueuedState.Waiting, QueuedState.Sending, QueuedState.NotSent })
        {
            var row = Assert.Single(Build(Queued(state: state, editable: false)));
            QueuedAction[] expected = state == QueuedState.NotSent
                ? [QueuedAction.SendAgain, QueuedAction.Discard]
                : [QueuedAction.SendNow, QueuedAction.Cancel];

            Assert.DoesNotContain(QueuedAction.Edit, row.Actions);
            Assert.Equal(expected, row.Actions);
        }
    }

    [Fact]
    public void An_unconfirmed_message_no_composer_can_hold_offers_the_same_two_answers()
    {
        var row = Assert.Single(Build(Queued(state: QueuedState.Unconfirmed, editable: false)));

        Assert.Equal([QueuedAction.MarkSent, QueuedAction.ConfirmNotSent], row.Actions);
    }

    [Fact]
    public void Sending_again_is_an_answer_on_an_unconfirmed_row_and_a_retry_on_a_refused_one()
    {
        // The two items read the same. Only the answer may send an unconfirmed message again,
        // and the core refuses it on any other state, so a row that changed under the click
        // cannot send twice.
        var answer = OutboxRows.IntentFor(QueuedAction.ConfirmNotSent, "acct-1", 7, Staging);
        var retry = OutboxRows.IntentFor(QueuedAction.SendAgain, "acct-1", 7, Staging);

        Assert.Equal(new OutboxIntent.ConfirmNotSent("acct-1", 7), answer);
        Assert.Equal(new OutboxIntent.SendNow("acct-1", 7), retry);
    }

    [Fact]
    public void Only_sending_an_unconfirmed_message_again_asks_first()
    {
        // If it did arrive, sending it again delivers it twice, so the user confirms. A refused
        // message did not go, so sending it again asks nothing.
        var asking = Enum.GetValues<QueuedAction>().Where(OutboxRows.NeedsConfirming);

        Assert.Equal([QueuedAction.ConfirmNotSent], asking);
        Assert.False(OutboxRows.NeedsConfirming(QueuedAction.SendAgain));
    }

    [Fact]
    public void Each_item_sends_its_own_intent_for_the_account_and_op_it_names()
    {
        Assert.Equal(
            new OutboxIntent.SendNow("acct-2", 3),
            OutboxRows.IntentFor(QueuedAction.SendNow, "acct-2", 3, Staging));
        Assert.Equal(
            new OutboxIntent.Cancel("acct-2", 3),
            OutboxRows.IntentFor(QueuedAction.Cancel, "acct-2", 3, Staging));
        Assert.Equal(
            new OutboxIntent.Cancel("acct-2", 3),
            OutboxRows.IntentFor(QueuedAction.Discard, "acct-2", 3, Staging));
        Assert.Equal(
            new OutboxIntent.ConfirmSent("acct-2", 3),
            OutboxRows.IntentFor(QueuedAction.MarkSent, "acct-2", 3, Staging));
    }

    [Fact]
    public void Edit_names_where_the_core_writes_the_messages_files()
    {
        // The composer that opens must hold every file, because its first save replaces the
        // draft; the core can only stage them where it is told to.
        Assert.Equal(
            new OutboxIntent.Edit("acct-2", 3, Staging),
            OutboxRows.IntentFor(QueuedAction.Edit, "acct-2", 3, Staging));
    }

    [Fact]
    public void Only_cancelling_and_discarding_are_set_apart_as_destructive()
    {
        var destructive = Enum.GetValues<QueuedAction>().Where(OutboxRows.IsDestructive);

        Assert.Equal([QueuedAction.Cancel, QueuedAction.Discard], destructive);
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
