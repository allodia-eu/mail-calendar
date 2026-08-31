// The custom URI scheme each build shape claims. The assertions here stand in for a failure that is
// otherwise silent until a user tries to sign in:
//
//   - the PACKAGED scheme must equal what Package.appxmanifest declares. The manifest is the only
//     thing that registers the protocol for the shipped app, so a rename on one side alone means
//     the OS delivers the redirect to nothing and sign-in dies in the Store build, while every
//     dev build, which registers its scheme at runtime from this same constant, keeps working.
//     That is the worst shape of bug: invisible exactly where it is tested.
//   - the two shapes must NOT be equal. Windows registers a protocol per user, not per build, so a
//     developer machine with the Store app installed and a dev build run had both claiming one
//     scheme: the OS answered with a "select an app" picker for a redirect carrying a live auth
//     code (observed 2026-07-21), and an "Always" would have routed the shipped app's sign-ins to
//     a debug build permanently.
//
// The committed manifest carries the UNBRANDED identity and package.ps1 rewrites it for the build
// (docs/branding.md), so the first invariant is checked in two halves that meet in the middle:
// here, that the code takes its scheme from the same application id the manifest is rewritten
// with; and in scripts/dev/tests/test_msix_manifest.py, that the rewrite puts that id into the
// manifest and touches no other protocol. Neither half alone would catch a rename.
//
// The manifest is copied beside the test assembly by the .csproj, so this reads a real file rather
// than a fixture that could drift from it.
using System.IO;
using System.Text.RegularExpressions;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class OAuthSchemeTests
{
    private static string Manifest() =>
        File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Package.appxmanifest"));

    [Fact]
    public void The_packaged_scheme_is_the_application_id_and_not_a_second_copy_of_it()
    {
        // The manifest is rewritten from Brand.AppId at packaging time, so a constant written out
        // by hand here would agree with it only until the app was re-branded.
        Assert.Equal(Brand.AppId, OAuthScheme.Packaged);
    }

    [Fact]
    public void The_manifest_claims_exactly_one_scheme_of_our_own()
    {
        // `mailto` is the OS's scheme, which we register as a handler for; everything else in that
        // list is ours and moves with the application id. A second one added here and forgotten by
        // the rewrite would ship a dead protocol registration.
        var declared = Regex.Matches(Manifest(), "<uap:Protocol Name=\"([^\"]+)\">")
            .Select(match => match.Groups[1].Value)
            .Where(name => name != "mailto")
            .ToList();

        Assert.Single(declared);
    }

    [Fact]
    public void The_dev_scheme_is_never_the_one_the_shipped_app_claims()
    {
        Assert.NotEqual(OAuthScheme.Packaged, OAuthScheme.Unpackaged);
        // And the manifest must not declare the dev scheme, a packaged build claiming it would
        // put the Store app back in the picker it was split out of. Checked as an exact
        // declaration, not a substring: the packaged scheme is a prefix of the dev one.
        Assert.DoesNotContain($"<uap:Protocol Name=\"{OAuthScheme.Unpackaged}\">", Manifest());
    }

    [Fact]
    public void A_build_claims_the_scheme_that_matches_its_packaging()
    {
        Assert.Equal(OAuthScheme.Packaged, OAuthScheme.For(packaged: true));
        Assert.Equal(OAuthScheme.Unpackaged, OAuthScheme.For(packaged: false));
    }
}
