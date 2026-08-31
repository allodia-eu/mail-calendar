// The email-first detection routing + connect-gating, as arithmetic, the WinUI-free half of the
// flow (AccountDetectForm), the Windows twin of Android's AccountSetupDetectTest and Apple's
// AccountSetupDetectTests. Two things are load-bearing: how a detection result maps to a routed,
// prefilled tab, and that an untrusted result cannot be connected until the user approves it.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AccountDetectFormTests
{
    private static DetectedServerRow Row(string protocol, string host, ushort port) =>
        new(protocol, host, port, "SSL/TLS", "alice@example.com");

    private static SetupRecommendation Imap(
        bool trusted, bool withSmtp = true, string? caldavUrl = null,
        ConnectionSecurity security = ConnectionSecurity.ImplicitTls) =>
        new SetupRecommendation.Imap(
            "alice@example.com", "imap.example.com", withSmtp ? "smtp.example.com" : null,
            security, security,
            Row("IMAP", "imap.example.com", 993),
            withSmtp ? Row("SMTP", "smtp.example.com", 465) : null,
            caldavUrl,
            trusted, "https://autoconfig.example.com/mail/config-v1.1.xml");

    [Fact]
    public void Jmap_routes_to_a_prefilled_jmap_tab()
    {
        var route = AccountDetectForm.Route(new SetupRecommendation.Jmap(
            "alice@example.com", "https://example.com", true, "https://example.com/.well-known/jmap"));
        Assert.False(route.IsManual);
        Assert.Equal(DetectTab.Jmap, route.Tab);
        Assert.Equal("alice@example.com", route.Email);
        Assert.Equal("https://example.com", route.JmapServer);
        Assert.False(route.NeedsApproval);
    }

    [Fact]
    public void Imap_routes_to_a_prefilled_imap_tab_with_the_hosts()
    {
        var route = AccountDetectForm.Route(Imap(trusted: true));
        Assert.Equal(DetectTab.Imap, route.Tab);
        Assert.Equal("imap.example.com", route.ImapHost);
        Assert.Equal("smtp.example.com", route.SmtpHost);
    }

    [Fact]
    public void An_untrusted_result_needs_approval()
    {
        Assert.True(AccountDetectForm.Route(Imap(trusted: false)).NeedsApproval);
    }

    [Fact]
    public void Imap_carries_the_detected_connection_security()
    {
        // Implicit TLS passes through; a STARTTLS detection routes STARTTLS so the engine dials it.
        var implicitRoute = AccountDetectForm.Route(Imap(trusted: true));
        Assert.Equal(ConnectionSecurity.ImplicitTls, implicitRoute.ImapSecurity);
        Assert.Equal(ConnectionSecurity.ImplicitTls, implicitRoute.SmtpSecurity);

        var starttlsRoute = AccountDetectForm.Route(Imap(trusted: true, security: ConnectionSecurity.StartTls));
        Assert.Equal(ConnectionSecurity.StartTls, starttlsRoute.ImapSecurity);
        Assert.Equal(ConnectionSecurity.StartTls, starttlsRoute.SmtpSecurity);
    }

    [Fact]
    public void A_discovered_caldav_endpoint_prefills_the_calendar_field()
    {
        var route = AccountDetectForm.Route(Imap(trusted: true, caldavUrl: "https://caldav.soverin.net/calendars"));
        Assert.Equal("https://caldav.soverin.net/calendars", route.CaldavUrl);
    }

    [Fact]
    public void No_caldav_leaves_the_calendar_field_empty()
    {
        Assert.Equal(string.Empty, AccountDetectForm.Route(Imap(trusted: true)).CaldavUrl);
    }

    [Fact]
    public void Microsoft_routes_to_the_microsoft_tab()
    {
        var route = AccountDetectForm.Route(new SetupRecommendation.Microsoft("alice@example.com"));
        Assert.Equal(DetectTab.Microsoft, route.Tab);
        Assert.False(route.IsManual);
    }

    [Fact]
    public void A_manual_result_carries_its_reason()
    {
        var route = AccountDetectForm.Route(new SetupRecommendation.Manual(MissReason.OauthOnlyProvider));
        Assert.True(route.IsManual);
        Assert.Equal(MissReason.OauthOnlyProvider, route.Reason);
    }

    [Fact]
    public void Untrusted_settings_cannot_connect_without_approval()
    {
        // IMAP with a password, but the result is untrusted: Connect stays disabled until approved.
        Assert.False(AccountDetectForm.CanConnect(
            DetectTab.Imap, needsApproval: true, approved: false,
            "imap.example.com", "alice@example.com", "secret", ""));
        Assert.True(AccountDetectForm.CanConnect(
            DetectTab.Imap, needsApproval: true, approved: true,
            "imap.example.com", "alice@example.com", "secret", ""));
    }

    [Fact]
    public void Imap_connect_needs_a_host_email_and_password()
    {
        Assert.False(AccountDetectForm.CanConnect(
            DetectTab.Imap, needsApproval: false, approved: false,
            "imap.example.com", "alice@example.com", "", ""));
        Assert.True(AccountDetectForm.CanConnect(
            DetectTab.Imap, needsApproval: false, approved: false,
            "imap.example.com", "alice@example.com", "secret", ""));
    }

    [Fact]
    public void Jmap_connect_needs_a_secret()
    {
        Assert.False(AccountDetectForm.CanConnect(
            DetectTab.Jmap, needsApproval: false, approved: false,
            "", "alice@example.com", "", ""));
        // One secret field: an API token gates Connect exactly as a password does.
        Assert.True(AccountDetectForm.CanConnect(
            DetectTab.Jmap, needsApproval: false, approved: false,
            "", "alice@example.com", "", "tok_123"));
    }
}
