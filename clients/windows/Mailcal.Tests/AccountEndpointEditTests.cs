// Editing an account's servers in Settings (docs/accounts.md rule 14): what the fields show for a
// stored address, and what a save sends back.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AccountEndpointEditTests
{
    private static ushort Imap(ConnectionSecurity security) =>
        security == ConnectionSecurity.ImplicitTls ? (ushort)993 : (ushort)143;

    private static ushort Smtp(ConnectionSecurity security) =>
        security == ConnectionSecurity.ImplicitTls ? (ushort)465 : (ushort)587;

    private static AccountEndpoints Mailbox() => new(
        ImapHost: "imap.example.org",
        ImapSecurity: ConnectionSecurity.ImplicitTls,
        SmtpHost: "smtp.example.org",
        SmtpSecurity: ConnectionSecurity.StartTls,
        CaldavUrl: null,
        CarddavUrl: null,
        Username: "alice",
        Password: null);

    private static EndpointFields Fields(string password = "") => new(
        "imap.example.org", null, "smtp.example.org", null, null, null, "alice", password);

    [Fact]
    public void AnEmptyPasswordKeepsTheStoredOneAndATypedOneReplacesIt()
    {
        Assert.Equal(Mailbox(), AccountEndpointEdit.Edited(Mailbox(), Fields()));

        var changed = Fields("new secret") with { ImapSecurity = ConnectionSecurity.StartTls };
        var endpoints = AccountEndpointEdit.Edited(Mailbox(), changed);
        Assert.Equal("new secret", endpoints.Password);
        Assert.Equal(ConnectionSecurity.StartTls, endpoints.ImapSecurity);
    }

    [Fact]
    public void AServerTheAccountDoesNotHaveIsNeverAddedByTheForm()
    {
        var fields = new EndpointFields(null, null, null, null, "https://dav.example.org", null, "alice", "");
        var endpoints = AccountEndpointEdit.Edited(Mailbox(), fields);
        Assert.Null(endpoints.CaldavUrl);
        // And a server with no field keeps what it had.
        Assert.Equal("imap.example.org", endpoints.ImapHost);
    }

    [Fact]
    public void ADefaultPortIsStoredBare()
    {
        var tls = ConnectionSecurity.ImplicitTls;
        var starttls = ConnectionSecurity.StartTls;
        Assert.Equal("smtp.example.org", AccountEndpointEdit.Joined("smtp.example.org", "587", starttls, Smtp));
        Assert.Equal("imap.example.org", AccountEndpointEdit.Joined("imap.example.org", "993", tls, Imap));
        Assert.Equal("imap.example.org", AccountEndpointEdit.Joined("imap.example.org", "", tls, Imap));
        Assert.Equal("imap.example.org:1143", AccountEndpointEdit.Joined(" imap.example.org ", "1143", tls, Imap));
        // A port that is the standard one for the other security is not this one's.
        Assert.Equal("imap.example.org:143", AccountEndpointEdit.Joined("imap.example.org", "143", tls, Imap));
        Assert.Equal("imap.example.org", AccountEndpointEdit.Joined("imap.example.org", "143", starttls, Imap));
    }

    [Fact]
    public void APortWrittenIntoTheHostWins()
    {
        // As the setup form dials it: two ports would be a contradiction, and the one in the host
        // field is the one the person can see beside the name.
        Assert.Equal(
            "mail.example:2993",
            AccountEndpointEdit.Joined("mail.example:2993", "1234", ConnectionSecurity.ImplicitTls, Imap));
    }
}
