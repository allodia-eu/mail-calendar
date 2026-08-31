// The debug-only refusal to send outside the local harness. This exists because it already went
// wrong once: replying to a seeded fixture pre-fills a real-looking external address, the harness
// accepted the send, and its outbound queue then retried delivery to that domain for days.
//
// Pure BCL by design, so the parsing rule, including the "unreadable means external" bias, is
// pinned here rather than trusted at the moment someone is looking at a message, not an address.

using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class HarnessRecipientGateTests
{
    [Fact]
    public void HarnessLocalRecipientsArePermitted() =>
        Assert.Empty(HarnessRecipientGate.ExternalRecipients("alice@test.local", "bob@test.local", null));

    [Fact]
    public void AnExternalRecipientIsReported()
    {
        var external = HarnessRecipientGate.ExternalRecipients("news@example.com", null, null);
        Assert.Equal(new[] { "news@example.com" }, external);
    }

    // The exact shape that caused the incident: a local To with a fixture address left in Cc.
    [Fact]
    public void AnExternalAddressLeftInCcIsCaught()
    {
        var external = HarnessRecipientGate.ExternalRecipients(
            "bob@test.local", "ahmed.elamrani@example.eu", null);
        Assert.Equal(new[] { "ahmed.elamrani@example.eu" }, external);
    }

    // A reply pre-fills `Name <addr>`, not a bare address.
    [Fact]
    public void ADisplayNameFormIsParsed()
    {
        Assert.Empty(HarnessRecipientGate.ExternalRecipients("Alice Tester <alice@test.local>", null, null));
        Assert.Single(HarnessRecipientGate.ExternalRecipients("Newsletter <news@example.com>", null, null));
    }

    [Fact]
    public void EveryExternalRecipientInAListIsReported()
    {
        var external = HarnessRecipientGate.ExternalRecipients(
            "alice@test.local, news@example.com; ahmed@example.eu", null, null);
        Assert.Equal(new[] { "news@example.com", "ahmed@example.eu" }, external);
    }

    // Biased toward refusing: something we cannot read as a local address blocks the send. A
    // permissive parser here fails in the direction that actually costs something.
    [Theory]
    [InlineData("not-an-address")]
    [InlineData("alice@test.local.evil.com")]
    [InlineData("alice@")]
    public void AnUnreadableOrLookalikeEntryCountsAsExternal(string entry) =>
        Assert.Single(HarnessRecipientGate.ExternalRecipients(entry, null, null));

    [Fact]
    public void TheDomainCheckIsCaseInsensitive() =>
        Assert.Empty(HarnessRecipientGate.ExternalRecipients("Alice@TEST.LOCAL", null, null));
}
