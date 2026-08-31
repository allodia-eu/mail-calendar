// The embedded-image gate (docs/signatures.md). A signature rides in EVERY message its account
// sends, and base64 adds a third on top, so the 512 KB cap is a real limit rather than a formality,
// and it is enforced HERE, at the picker, because that is the only place the user can still be told
// which problem it is. The core does not police it, so nothing else would catch a regression.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SignatureImageTests
{
    [Fact]
    public void AnImageUnderTheCapBecomesADataUri()
    {
        var outcome = SignatureImage.From(new byte[] { 1, 2, 3 }, "image/png", "logo");
        var url = Assert.IsType<SignatureImageOutcome.DataUrl>(outcome);
        Assert.Equal("data:image/png;base64,AQID", url.Value);
        Assert.Equal("logo", url.AltText);
    }

    // Exactly at the cap is allowed; one byte past it is not, the boundary is where an off-by-one
    // would otherwise sit unnoticed.
    [Fact]
    public void TheCapIsInclusive() =>
        Assert.IsType<SignatureImageOutcome.DataUrl>(
            SignatureImage.From(new byte[SignatureImage.LimitBytes], "image/png", "logo"));

    [Fact]
    public void OneBytePastTheCapIsRefusedWithTheLimit()
    {
        var outcome = SignatureImage.From(new byte[SignatureImage.LimitBytes + 1], "image/png", "logo");
        var tooLarge = Assert.IsType<SignatureImageOutcome.TooLarge>(outcome);
        // The message names the cap, so the user knows what to pick instead.
        Assert.Equal(SignatureImage.LimitBytes, tooLarge.LimitBytes);
    }

    // Anything that is not an image is refused here rather than embedded: the editor only accepts
    // `data:image/` and would silently drop it, leaving the user staring at an editor that did
    // nothing. A `data:text/html` would additionally be an executable document.
    [Theory]
    [InlineData("text/html")]
    [InlineData("application/pdf")]
    [InlineData(null)]
    public void ANonImageIsRefused(string? mediaType) =>
        Assert.IsType<SignatureImageOutcome.Failed>(
            SignatureImage.From(new byte[] { 1, 2, 3 }, mediaType, "file"));

    [Fact]
    public void AnEmptyFileIsRefused() =>
        Assert.IsType<SignatureImageOutcome.Failed>(
            SignatureImage.From(Array.Empty<byte>(), "image/png", "logo"));

    // The size check runs BEFORE the media-type one, so an oversized file is reported as oversized
    // rather than as unreadable, telling the user the wrong thing sends them to the wrong fix.
    [Fact]
    public void SizeIsReportedBeforeType() =>
        Assert.IsType<SignatureImageOutcome.TooLarge>(
            SignatureImage.From(new byte[SignatureImage.LimitBytes + 1], "text/html", "file"));

    // Some pickers report the type upper-cased; a case-sensitive prefix check would refuse a valid
    // PNG, which reads to the user as "that image couldn't be read".
    [Fact]
    public void TheMediaTypeCheckIsCaseInsensitive()
    {
        var outcome = SignatureImage.From(new byte[] { 1, 2, 3 }, "IMAGE/PNG", "logo");
        var url = Assert.IsType<SignatureImageOutcome.DataUrl>(outcome);
        Assert.StartsWith("data:image/png;base64,", url.Value, StringComparison.Ordinal);
    }
}
