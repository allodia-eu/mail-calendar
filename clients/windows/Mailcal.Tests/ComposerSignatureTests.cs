// The composer's signature rules (docs/signatures.md), pinned. Every one of these fails silently in
// the running app, which is why they are here rather than left to a hand test:
//
//  * A mis-mapped slot sends the reply signature on a new message. Nothing on screen says so, the
//    user sees *a* signature, and it surfaces when a recipient mentions it.
//  * A resolution that re-resolves an explicit choice undoes a deliberate act at the moment the user
//    changes sender, which is exactly when they are not looking at the signature.
//  * A seed payload with the wrong key names reaches `setComposerSignature` as an object with no
//    `body_html`, which the editor reads as "remove the signature", so the message goes out with
//    none at all, and the composer looks right until you scroll.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class ComposerSignatureTests
{
    // A reply, a reply-all and a forward share ONE slot, Outlook's grouping. All three continue an
    // existing message, and splitting them produces a setting nobody sets.
    [Theory]
    [InlineData(RichComposeKind.Reply)]
    [InlineData(RichComposeKind.ReplyAll)]
    [InlineData(RichComposeKind.Forward)]
    public void ContinuingKindsShareTheReplyForwardSlot(RichComposeKind kind) =>
        Assert.Equal(SignatureSlotKind.ReplyForward, ComposerSignatures.SlotFor(kind));

    [Fact]
    public void ANewMessageUsesTheNewMessageSlot() =>
        Assert.Equal(SignatureSlotKind.NewMessage, ComposerSignatures.SlotFor(RichComposeKind.New));

    // No explicit choice means FOLLOW THE ACCOUNT: the account's assignment for this kind's slot.
    [Fact]
    public void WithNoChoiceTheAccountsAssignmentIsUsed()
    {
        var resolved = Resolve(choice: null, account: "work", kind: RichComposeKind.New);
        Assert.Equal("work/NewMessage", resolved?.Id);
    }

    [Fact]
    public void WithNoChoiceAReplyReadsTheReplyForwardSlot()
    {
        var resolved = Resolve(choice: null, account: "work", kind: RichComposeKind.Reply);
        Assert.Equal("work/ReplyForward", resolved?.Id);
    }

    // Nothing to resolve against before the From picker has an account.
    [Fact]
    public void WithNoAccountThereIsNoSignature() =>
        Assert.Null(Resolve(choice: null, account: null, kind: RichComposeKind.New));

    // "None" in the picker is an EXPLICIT choice, not the absence of one, which is what stops the
    // next From change putting a signature back on a message the user just took it off.
    [Fact]
    public void AnExplicitNoneWinsOverTheAccountsAssignment() =>
        Assert.Null(Resolve(new SignatureChoice(null), account: "work", kind: RichComposeKind.New));

    [Fact]
    public void AnExplicitChoiceNamesItsOwnSignature()
    {
        var resolved = Resolve(new SignatureChoice("chosen"), account: "work", kind: RichComposeKind.New);
        Assert.Equal("chosen", resolved?.Id);
    }

    // The rule the setting exists for, in both directions: with no explicit choice the signature
    // follows the sender, and with one it does not.
    [Fact]
    public void ChangingSenderReResolvesUntilTheUserPicks()
    {
        var choice = (SignatureChoice?)null;
        Assert.Equal("work/NewMessage", Resolve(choice, "work", RichComposeKind.New)?.Id);
        Assert.Equal("home/NewMessage", Resolve(choice, "home", RichComposeKind.New)?.Id);
    }

    [Fact]
    public void AnExplicitChoiceSurvivesAChangeOfSender()
    {
        var choice = new SignatureChoice("chosen");
        Assert.Equal("chosen", Resolve(choice, "work", RichComposeKind.New)?.Id);
        Assert.Equal("chosen", Resolve(choice, "home", RichComposeKind.New)?.Id);
    }

    // The payload the editor seam round-trips. The key names are the Rust field names verbatim: the
    // editor emits the same two back inside the Signature block on submit.
    [Fact]
    public void TheSeedCarriesBothBodiesUnderTheRustFieldNames()
    {
        var json = ComposerSignatures.SeedJson(new SignatureBody("id", "<p>Sam</p>", "Sam"));
        Assert.Equal("{\"body_html\":\"\\u003Cp\\u003ESam\\u003C/p\\u003E\",\"body_plain\":\"Sam\"}", json);
    }

    // Null is how "no signature" reaches the seam, which reads it as "remove the region", as
    // opposed to an object whose body_html is empty, which would be a signature that is blank.
    [Fact]
    public void NoSignatureSeedsNothingAtAll() => Assert.Null(ComposerSignatures.SeedJson(null));

    // A stand-in core: an account's slot resolves to "<account>/<slot>", and a named signature to
    // its own id, so each assertion can name which lookup answered.
    private static SignatureBody? Resolve(SignatureChoice? choice, string? account, RichComposeKind kind) =>
        ComposerSignatures.Resolve(
            choice,
            account,
            kind,
            (a, slot) => new SignatureBody($"{a}/{slot}", "<p>account</p>", "account"),
            id => new SignatureBody(id, "<p>named</p>", "named"));
}
