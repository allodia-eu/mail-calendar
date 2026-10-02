// What a refused certificate claims about itself, as the lines the setup form shows beside the
// acceptance box (docs/certificate-exceptions.md). Split from AccountSetupView.xaml.cs by
// responsibility (and to keep it under the 500-line limit); nothing here reads the view.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class AccountSetupView
{
    /// <summary>
    /// What the certificate claims about itself, one claim per line. Absent claims are left out
    /// rather than shown empty; the fingerprint is always there, because it is taken over the
    /// bytes rather than read out of them. Dates are formatted here, from the epoch seconds the
    /// core sends (docs/timestamps.md).
    /// </summary>
    private static string CertificateClaimLines(RejectedCertificate certificate)
    {
        var lines = new List<string>();
        var subject = Party(certificate.SubjectCommonName, certificate.SubjectOrganisation);
        if (subject is not null)
        {
            lines.Add($"{L10n.SetupCertificateIssuedTo()}: {subject}");
        }
        var issuer = Party(certificate.IssuerCommonName, certificate.IssuerOrganisation);
        if (issuer is not null)
        {
            lines.Add($"{L10n.SetupCertificateIssuedBy()}: {issuer}");
        }
        if (certificate.NotBefore is { } from && certificate.NotAfter is { } until)
        {
            lines.Add($"{L10n.SetupCertificateValid()}: {Day(from)} – {Day(until)}");
        }
        lines.Add($"{L10n.SetupCertificateFingerprint()}: {certificate.Sha256}");
        return string.Join(Environment.NewLine, lines);
    }

    // The process culture is already pinned to the app's language choice (AppCulture), so the
    // long-date format follows the words on screen rather than the host's region.
    private static string Day(long epochSeconds) =>
        DateTimeOffset.FromUnixTimeSeconds(epochSeconds).ToLocalTime().ToString("D");

    /// <summary>
    /// "name (organisation)", or whichever of the two the certificate carries. <c>null</c> when it
    /// names neither, which is when there is nothing to show rather than an empty line.
    /// </summary>
    private static string? Party(string? commonName, string? organisation) => (commonName, organisation) switch
    {
        (not null, not null) => $"{commonName} ({organisation})",
        (not null, null) => commonName,
        (null, not null) => organisation,
        _ => null,
    };
}
