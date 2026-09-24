// What a new-mail pass puts on the desktop (docs/background-sync.md). Two of the three rules
// here fail silently in the running app, which is why they are pinned rather than trusted:
//
//   - one notification PER MESSAGE, keyed by the message. A per-account tag would be replaced by
//     the next pass's toast, and because the core's marks have already advanced past the mail it
//     reported, the message it replaced is gone with nothing looking wrong.
//   - the overflow summary carries its own words. Reusing the unknown-sender line made a single
//     hidden message read as one more subject-less message.
//
// The third, that a notification is never blank, is the one a screenshot would catch, and only if
// somebody happened to receive mail from a header with no display name while looking.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class NewMailNoticesTests
{
    private const string Unknown = "New message";

    private static string More(int count) => $"+{count} more";

    private static NewMailPreview Preview(
        string key, string sender, string? name, string subject, string preview = "") =>
        new(
            Sender: sender,
            SenderName: name,
            Subject: subject,
            Preview: preview,
            Received: string.Empty,
            MessageKey: key);

    private static BackgroundSyncOutcome Outcome(params AccountNewMail[] accounts) =>
        new(Accounts: accounts, TimedOut: false);

    [Fact]
    public void A_message_says_who_it_is_from_what_it_is_about_and_how_it_begins()
    {
        var outcome = Outcome(new AccountNewMail(
            AccountId: "acct",
            AccountLabel: "me@example.test",
            NewCount: 1,
            Messages:
            [
                Preview("m1", "jane@example.test", "Jane", "Quarterly report", "The numbers you asked for."),
            ]));

        var notices = NewMailNotices.For(outcome, Unknown, More);

        Assert.Single(notices);
        Assert.Equal("Jane", notices[0].Title);
        Assert.Equal("Quarterly report", notices[0].Body);
        Assert.Equal("The numbers you asked for.", notices[0].Preview);
    }

    [Fact]
    public void A_message_with_no_snippet_yet_carries_none_rather_than_an_empty_line()
    {
        // An IMAP account's snippet is computed by the body sync, so the pass that commits the
        // headers has none. The notification is then the two lines it always was.
        var outcome = Outcome(new AccountNewMail(
            AccountId: "acct",
            AccountLabel: "me@example.test",
            NewCount: 1,
            Messages: [Preview("m1", "jane@example.test", "Jane", "Quarterly report")]));

        Assert.Equal(string.Empty, NewMailNotices.For(outcome, Unknown, More)[0].Preview);
    }

    [Fact]
    public void A_sender_with_no_display_name_falls_back_to_the_address_then_to_the_fallback_line()
    {
        var outcome = Outcome(new AccountNewMail(
            AccountId: "acct",
            AccountLabel: "me@example.test",
            NewCount: 3,
            Messages:
            [
                Preview("m1", "jane@example.test", null, "No name"),
                Preview("m2", "jane@example.test", "   ", "Blank name"),
                Preview("m3", string.Empty, null, "No sender at all"),
            ]));

        var notices = NewMailNotices.For(outcome, Unknown, More);

        Assert.Equal("jane@example.test", notices[0].Title);
        Assert.Equal("jane@example.test", notices[1].Title);
        Assert.Equal(Unknown, notices[2].Title);
    }

    [Fact]
    public void Every_message_gets_its_own_notification_identity()
    {
        var outcome = Outcome(new AccountNewMail(
            AccountId: "acct",
            AccountLabel: "me@example.test",
            NewCount: 2,
            Messages:
            [
                Preview("m1", "a@example.test", null, "One"),
                Preview("m2", "b@example.test", null, "Two"),
            ]));

        var notices = NewMailNotices.For(outcome, Unknown, More);

        Assert.Equal(["mailcal-m1", "mailcal-m2"], notices.Select(notice => notice.Tag));
        Assert.All(notices, notice => Assert.Equal("acct", notice.Group));
    }

    [Fact]
    public void Only_the_messages_beyond_the_cap_earn_a_summary_and_it_has_its_own_words()
    {
        // The core caps its previews; NewCount is what actually arrived.
        var outcome = Outcome(new AccountNewMail(
            AccountId: "acct",
            AccountLabel: "me@example.test",
            NewCount: 3,
            Messages: [Preview("m1", "a@example.test", null, "One")]));

        var notices = NewMailNotices.For(outcome, Unknown, More);

        Assert.Equal(2, notices.Count);
        Assert.Equal("+2 more", notices[1].Title);
        Assert.Equal("me@example.test", notices[1].Body);
        Assert.Equal("mailcal-account-acct", notices[1].Tag);
        // The summary stands for messages it does not name, so it quotes none of them.
        Assert.Equal(string.Empty, notices[1].Preview);
    }

    [Fact]
    public void A_pass_that_fitted_says_nothing_extra()
    {
        var outcome = Outcome(new AccountNewMail(
            AccountId: "acct",
            AccountLabel: "me@example.test",
            NewCount: 1,
            Messages: [Preview("m1", "a@example.test", null, "One")]));

        Assert.Single(NewMailNotices.For(outcome, Unknown, More));
    }

    [Fact]
    public void Accounts_are_grouped_separately_so_one_cannot_replace_anothers_summary()
    {
        var outcome = Outcome(
            new AccountNewMail(
                AccountId: "first",
                AccountLabel: "one@example.test",
                NewCount: 2,
                Messages: [Preview("m1", "a@example.test", null, "One")]),
            new AccountNewMail(
                AccountId: "second",
                AccountLabel: "two@example.test",
                NewCount: 2,
                Messages: [Preview("m2", "b@example.test", null, "Two")]));

        var notices = NewMailNotices.For(outcome, Unknown, More);

        Assert.Equal(
            ["mailcal-m1", "mailcal-account-first", "mailcal-m2", "mailcal-account-second"],
            notices.Select(notice => notice.Tag));
        Assert.Equal(
            ["first", "first", "second", "second"],
            notices.Select(notice => notice.Group));
    }

    [Fact]
    public void A_pass_that_found_nothing_raises_nothing()
    {
        Assert.Empty(NewMailNotices.For(Outcome(), Unknown, More));
    }

    [Fact]
    public void A_message_names_the_message_a_click_opens_and_the_summary_names_none()
    {
        // The pair the core keys mail on, which is what Intent::OpenMessage takes. A summary
        // stands for messages it does not name, so it has none to open and brings the app forward
        // alone; the empty key is what says so.
        var outcome = Outcome(new AccountNewMail(
            AccountId: "acct",
            AccountLabel: "me@example.test",
            NewCount: 3,
            Messages: [Preview("m1", "a@example.test", null, "One")]));

        var notices = NewMailNotices.For(outcome, Unknown, More);

        Assert.Equal("m1", notices[0].MessageKey);
        Assert.Equal("acct", notices[0].Group);
        Assert.Equal(string.Empty, notices[1].MessageKey);
    }

    [Theory]
    [InlineData("acct", "m1")]
    // A launch argument list is `key=value;key=value`, and Microsoft Graph's message keys are
    // base64 WITH padding, so a key carrying `=` (or `;`) is a key that changes where the list is
    // cut. It has to survive whole or that provider's notifications quietly open nothing.
    [InlineData("acct", "AAMkAGI2THVSAAA=")]
    [InlineData("acct", "one;two=three")]
    // Nothing says an account id is plain either, and the account half is what the message key is
    // resolved within.
    [InlineData("me@example.test", "m1")]
    public void The_message_a_notification_names_survives_the_round_trip_through_its_arguments(
        string account, string key)
    {
        // Through the notice, so this pins the pair the poster actually writes rather than the
        // spelling of it: a key typo'd on one side is a click that silently opens nothing.
        var outcome = Outcome(new AccountNewMail(
            AccountId: account,
            AccountLabel: "me@example.test",
            NewCount: 1,
            Messages: [Preview(key, "a@example.test", null, "One")]));
        var notice = NewMailNotices.For(outcome, Unknown, More)[0];
        var (accountArgument, messageArgument) =
            new NotificationTarget(notice.Group, notice.MessageKey).Arguments;

        var target = NotificationTarget.From(new Dictionary<string, string>
        {
            [NotificationTarget.AccountArgument] = accountArgument,
            [NotificationTarget.MessageArgument] = messageArgument,
        });

        Assert.Equal(new NotificationTarget(account, key), target);
        // The two characters the argument list itself is cut on.
        Assert.DoesNotContain("=", accountArgument + messageArgument);
        Assert.DoesNotContain(";", accountArgument + messageArgument);
    }

    [Theory]
    // The summary's own shape: it carries neither argument, and so names no message.
    [InlineData(null, null)]
    // Half a pair resolves to no message at all, so acting on it would look like the click went
    // to the wrong message rather than to none.
    [InlineData("YWNjdA", null)]
    [InlineData(null, "bTE")]
    [InlineData("YWNjdA", "")]
    [InlineData("", "bTE")]
    // And an argument this app did not write, or one damaged in transit: the click then names no
    // message and opens the app, which is what it did before there was a deep link at all.
    [InlineData("YWNjdA", "!not base64url!")]
    public void Arguments_that_do_not_name_a_whole_message_name_none(string? account, string? message)
    {
        var arguments = new Dictionary<string, string>();
        if (account is not null)
        {
            arguments[NotificationTarget.AccountArgument] = account;
        }
        if (message is not null)
        {
            arguments[NotificationTarget.MessageArgument] = message;
        }

        Assert.Null(NotificationTarget.From(arguments));
    }
}
