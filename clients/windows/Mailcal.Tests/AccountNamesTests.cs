// What Settings names an account by where another account has its address (docs/accounts.md rule
// 8): a link, a picker and a removal's confirmation add what tells them apart, and an account's own
// kind line adds only a server's host.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AccountNamesTests
{
    internal static readonly AccountNameWords Words = new(
        kind => kind switch
        {
            AccountKind.Dav => "DAV",
            AccountKind.Imap => "IMAP",
            _ => kind.ToString(),
        },
        (address, detail) => $"{address} ({detail})");

    private static LinkedAccount Linked(AccountDistinction? distinction) =>
        new("dennis@example.nl@dav.example.nl", "dennis@example.nl", distinction);

    [Fact]
    public void AnAccountNoOtherSharesTheAddressOfIsNamedByItsAddress()
    {
        Assert.Equal("dennis@example.nl", AccountNames.Of(Linked(null), Words));
    }

    [Fact]
    public void AKindDistinctionNamesTheKind()
    {
        Assert.Equal(
            "dennis@example.nl (DAV)",
            AccountNames.Of(Linked(new AccountDistinction.Kind(AccountKind.Dav)), Words));
    }

    [Fact]
    public void AServerDistinctionNamesTheHost()
    {
        Assert.Equal(
            "dennis@example.nl (dav.example.nl)",
            AccountNames.Of(Linked(new AccountDistinction.Server("dav.example.nl")), Words));
    }

    [Fact]
    public void TheKindLineGainsTheHostForAServerDistinctionOnly()
    {
        var entry = AccountSettingsRulesTests.MailboxOnly();
        Assert.Equal("IMAP", AccountNames.KindLine(entry, Words));

        // The row says the kind already, so a kind distinction adds nothing to it.
        var kind = entry with { Distinction = new AccountDistinction.Kind(AccountKind.Imap) };
        Assert.Equal("IMAP", AccountNames.KindLine(kind, Words));

        var server = entry with { Distinction = new AccountDistinction.Server("mail.example.org") };
        Assert.Equal("IMAP · mail.example.org", AccountNames.KindLine(server, Words));
    }
}
