// The mail-link (`mailto:`) activation gate, docs/composer-security.md, Gate 12.
//
// Everything here fails SILENTLY in the app, which is why it is pinned rather than trusted. A
// scheme check that lets an OAuth redirect through swallows a sign-in and pops a composer over it;
// a manifest whose protocol name drifts from the code's leaves an app that Windows never offers as
// a mail handler at all, with nothing to see and nothing logged.
using System;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class MailLinkTests
{
    [Fact]
    public void A_mail_link_is_recognized_whatever_its_casing()
    {
        // A browser is not obliged to hand the scheme over lowercased, and RFC 6068 makes it
        // case-insensitive.
        Assert.True(MailLink.CarriesMailLink("mailto"));
        Assert.True(MailLink.CarriesMailLink("MAILTO"));
        Assert.True(MailLink.CarriesMailLink("MailTo"));
    }

    /// <summary>
    /// The bug this gate exists to prevent. Every browser sign-in returns through a protocol
    /// activation too, so a check on the activation KIND alone would read a redirect carrying a
    /// live auth code as a mail link, swallowing the sign-in and opening a composer in the middle
    /// of adding an account.
    /// </summary>
    [Fact]
    public void An_oauth_redirect_is_never_mistaken_for_a_mail_link()
    {
        Assert.False(MailLink.CarriesMailLink(OAuthScheme.Packaged));
        Assert.False(MailLink.CarriesMailLink(OAuthScheme.Unpackaged));
        Assert.False(MailLink.CarriesMailLink("https"));
        Assert.False(MailLink.CarriesMailLink(null));
        Assert.False(MailLink.CarriesMailLink(string.Empty));
    }

    [Fact]
    public void The_link_is_picked_out_of_a_command_line()
    {
        Assert.Equal(
            "mailto:bob@test.local",
            MailLink.FromArguments(new[] { @"C:\app\Mailcal.exe", "mailto:bob@test.local" }));
        Assert.Null(MailLink.FromArguments(new[] { @"C:\app\Mailcal.exe" }));
        Assert.Null(MailLink.FromArguments(new[] { @"C:\app\Mailcal.exe", "--calendar" }));
    }

    /// <summary>
    /// A redirected launch hands the command-line tail over as ONE unsplit string, so the whole
    /// line is the link. Splitting it on whitespace, which is enough for a bare <c>--calendar</c>
    /// flag, would truncate an unencoded subject at its first space and silently drop the rest of
    /// what the user was told they were sending.
    /// </summary>
    [Fact]
    public void An_unencoded_space_in_a_subject_survives_a_redirected_launch()
    {
        Assert.Equal(
            "mailto:bob@test.local?subject=Lunch on Friday",
            MailLink.FromArgumentLine("mailto:bob@test.local?subject=Lunch on Friday"));
        // Windows quotes an argument containing spaces; the quotes are the shell's, not the URI's.
        Assert.Equal(
            "mailto:bob@test.local?subject=Lunch on Friday",
            MailLink.FromArgumentLine("\"mailto:bob@test.local?subject=Lunch on Friday\""));
    }

    [Fact]
    public void A_launch_that_carries_no_link_yields_none()
    {
        Assert.Null(MailLink.FromArgumentLine(null));
        Assert.Null(MailLink.FromArgumentLine("   "));
        Assert.Null(MailLink.FromArgumentLine("--calendar"));
    }

    /// <summary>
    /// The coupling that makes any of this reachable, and which nothing else checks: Windows only
    /// offers the app as a mail handler because <c>Package.appxmanifest</c> declares a protocol of
    /// exactly the name the activation gate matches on. Drift either half and the app simply never
    /// receives a mail link, no error, nothing in the log.
    /// </summary>
    [Fact]
    public void The_app_manifest_registers_the_protocol_the_gate_matches_on()
    {
        var manifest = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Package.appxmanifest"));
        Assert.Contains(
            $"<uap:Protocol Name=\"{MailLink.Scheme}\">",
            manifest,
            StringComparison.Ordinal);
    }
}
