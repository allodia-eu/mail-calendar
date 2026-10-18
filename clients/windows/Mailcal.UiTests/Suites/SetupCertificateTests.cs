// A calendar server whose certificate does not verify, set up as an account without mail
// (docs/certificate-exceptions.md, docs/accounts.md): the refusal shows the certificate, Connect
// stays inert until it is accepted, and accepting it connects the account.
//
// WHY HERE. The core's half is unit-tested (the refusal carries the certificate; an acceptance is
// stored with the account), and the form's rules for the panel are WinUI. What neither can see is
// the refusal reaching the form at all on this route, which is how Android's panel was once
// written, tested and unreachable.
//
// The server is the harness's own CalDAV and CardDAV behind an HTTPS listener on 12443 whose
// certificate no debug build is handed (docker/stalwart/front/nginx.conf). The cases run in the
// order written, and the last removes the account, returning the window to the first-run screen.

using System.Net.Security;
using System.Net.Sockets;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[TestCaseOrderer(DeclarationOrderer.Name, DeclarationOrderer.Assembly)]
public sealed class SetupCertificateTests(SetupHarnessApp harness) : IClassFixture<SetupHarnessApp>
{
    private const string Server = "https://localhost:12443";

    private void Case(string name, Action<App, SetupFlow> body) =>
        harness.Case(name, app => body(app, new SetupFlow(app)), app => app.CloseDialogs());

    /// <summary>The SHA-256 of the certificate the untrusted listener serves, read off the listener
    /// itself: what the panel has to match is what the server presented, and a fingerprint read from
    /// anywhere else proves only that two copies of one file agree.</summary>
    private static string ListenerFingerprint()
    {
        using var tcp = new TcpClient("127.0.0.1", 12443);
        using var tls = new SslStream(tcp.GetStream(), false, (_, _, _, _) => true);
        tls.AuthenticateAsClient("localhost");
        using var certificate = new X509Certificate2(tls.RemoteCertificate!);
        return certificate.GetCertHashString(HashAlgorithmName.SHA256);
    }

    private static Element Connect(App app) =>
        app.Agent.Find(Filter.ById("ConnectButton", "Button")) ?? throw new InvalidOperationException("the form has no Connect");

    [HarnessFact]
    public void TheRefusedCertificateIsShownFingerprintIncludedAndConnectWaitsForTheBox() =>
        Case(nameof(TheRefusedCertificateIsShownFingerprintIncludedAndConnectWaitsForTheBox), (app, flow) =>
        {
            flow.OpenManualSetup(Harness.Alice, Filter.ById("DavChoice"));
            app.Agent.SetValue(app.Agent.Find(Filter.ById("DavLogin", "Edit"))!, Harness.Alice);
            app.Agent.SetValue(app.Agent.Find(Filter.ById("DavPassword", "Edit"))!, "harness-alice-pw");
            app.Agent.SetValue(app.Agent.Find(Filter.ById("DavCaldavUrl", "Edit"))!, Server);
            app.Agent.SetValue(app.Agent.Find(Filter.ById("DavCarddavUrl", "Edit"))!, Server);
            app.Agent.Invoke(Connect(app));
            var claims = app.Agent.Wait(Filter.ById("CertificateClaims"), TimeSpan.FromSeconds(30));
            Assert.True(claims is not null,
                "docs/certificate-exceptions.md rule 3: a calendar server refused on its certificate shows that certificate, for an account without mail as for a mailbox");
            var shown = new string(claims!.Name.Where(Uri.IsHexDigit).ToArray()).ToUpperInvariant();
            var served = ListenerFingerprint();
            Assert.True(shown.Contains(served, StringComparison.Ordinal),
                $"rule 4: the panel's SHA-256 is the certificate the server presented ({served}). The panel reads: {claims.Name}");
            Assert.False(app.Agent.Read(Connect(app)).Element.Enabled, "rule 5: Connect stays inert until the box is ticked");
            app.Agent.Toggle(app.Agent.Find(Filter.ById("CertificateCheck"))!);
            Assert.True(App.Eventually(() => app.Agent.Read(Connect(app)).Element.Enabled, TimeSpan.FromSeconds(5)), "ticking the box is what lets Connect go");
            Assert.True(app.Agent.Read(app.Agent.Find(Filter.ById("DavCaldavUrl", "Edit"))!).Value == Server,
                "rule 6: the question is answered where it was asked, on the form as it was filled in");
        });

    [HarnessFact]
    public void AcceptingItConnectsTheAccountUsedForItsCalendarAndContactsAndNoMail() =>
        Case(nameof(AcceptingItConnectsTheAccountUsedForItsCalendarAndContactsAndNoMail), (app, flow) =>
        {
            app.Agent.Invoke(Connect(app));
            flow.CompleteSetupPrompts();
            var dialog = flow.OpenAccount(Harness.Alice);
            foreach (var use in new[] { "AccountUse_Calendar", "AccountUse_Contacts" })
            {
                var toggle = app.Agent.Find(new Filter { Id = use, Under = dialog });
                Assert.True(toggle is not null && app.Agent.Read(toggle).Toggle == "On",
                    $"the account connected through the accepted certificate and is used for its {use["AccountUse_".Length..]}");
            }
            var mail = app.Agent.Find(new Filter { Id = "AccountUse_Mail", Under = dialog });
            Assert.True(mail is null || app.Agent.Read(mail).Toggle != "On", "an account set up as calendar and contacts is used for no mail");
            var signInAgain = Catalog.Everywhere("signin_expired_action");
            Assert.False(app.Agent.FindAll(new Filter { Under = dialog }).Any(e => signInAgain.Contains(e.Name)),
                "and it is connected: nothing on its page asks to sign in again");
        });

    [HarnessFact]
    public void RemovingItReturnsTheWindowToFirstRunSetup() => Case(nameof(RemovingItReturnsTheWindowToFirstRunSetup), (_, flow) =>
        flow.RemoveOnlyAccount(Harness.Alice));
}
