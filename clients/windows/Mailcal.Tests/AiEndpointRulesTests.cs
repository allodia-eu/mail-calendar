// The own AI endpoint form's half of Settings → Advanced (docs/ai.md rules 6 and 7). What a save
// refuses and what it came to are the core's, pinned in mailcal-app's endpoint tests and
// mailcal-bindings' app_ai_tests.rs; what is pinned here is what the form does with them: the words
// beside each field, Save as drawn, the key it sends and which outcome it says, once.

using Allodia.Mailcal.Services;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class AiEndpointRulesTests
{
    private static readonly ControlState Free = new(true, false);
    private static readonly ControlState Running = new(false, true);

    private static readonly AiEndpointWords Words = new(
        "address missing", "address unusable", "needs https", "model missing", "where missing",
        "eu native only", "eu only", "key hint", "key kept", "key not stored");

    private static readonly AiEndpointGaps None = new(null, false, null);

    private static readonly AiEndpointTyped Typed = new("https://ai.example.eu/v1", string.Empty, "model", JurisdictionClass.EuNative);

    private static AiEndpointForm Form(AiEndpointOutcome? outcome = null) =>
        new(string.Empty, string.Empty, null, false, Free, null, outcome);

    private static AiEndpointOutcome Outcome(ulong serial, AiEndpointResult result) =>
        new(serial, AiEndpointChangeKind.Save, result);

    [Fact]
    public void EachGapIsSaidBesideItsField()
    {
        var lines = AiEndpointRules.Lines(
            new AiEndpointGaps(AiEndpointAddressGap.NeedsHttps, true, new AiEndpointDeclarationGap.Missing()), Typed, Words);
        Assert.Equal(new AiEndpointGapLines("needs https", "model missing", "where missing"), lines);
    }

    [Fact]
    public void AFormWithNothingGivenSaysNothing()
    {
        var everything = new AiEndpointGaps(AiEndpointAddressGap.Missing, true, new AiEndpointDeclarationGap.Missing());
        Assert.Equal(new AiEndpointGapLines(null, null, null),
            AiEndpointRules.Lines(everything, new AiEndpointTyped(" ", string.Empty, string.Empty, null), Words));
        Assert.Equal("address missing",
            AiEndpointRules.Lines(everything, new AiEndpointTyped(string.Empty, string.Empty, "m", null), Words).Address);
    }

    // The mode is the core's, and decides which refusal is said: each names what it admits. The
    // generated enum is internal, so a theory names it rather than taking it.
    [Theory]
    [InlineData("eu-native", "eu native only")]
    [InlineData("eu-hosted", "eu only")]
    public void ARefusedDeclarationSaysWhatTheModeAdmits(string mode, string said)
    {
        var inForce = mode == "eu-hosted" ? JurisdictionMode.EuHosted : JurisdictionMode.EuNative;
        var gaps = new AiEndpointGaps(null, false, new AiEndpointDeclarationGap.Refused(inForce, JurisdictionClass.NonEu));
        Assert.Equal(said, AiEndpointRules.Lines(gaps, Typed, Words).Where);
    }

    [Fact]
    public void SaveIsHeldWhileTheFieldsLeaveSomethingOutAndRunsAsTheRecordSays()
    {
        Assert.Equal(Free, AiEndpointRules.SaveState(Free, None));
        Assert.Equal(new ControlState(false, false), AiEndpointRules.SaveState(Free, None with { ModelMissing = true }));
        Assert.Equal(Running, AiEndpointRules.SaveState(Running, None));
    }

    [Fact]
    public void TheKeyFieldSaysWhetherOneIsKept()
    {
        Assert.Equal("key kept", AiEndpointRules.KeyNote(true, Words));
        Assert.Equal("key hint", AiEndpointRules.KeyNote(false, Words));
    }

    [Fact]
    public void AKeyTheKeystoreRefusedIsSaidOnce()
    {
        var outcomes = new AiEndpointOutcomes(Form());
        var refused = Outcome(1, new AiEndpointResult.KeyNotStored());
        Assert.True(outcomes.IsNew(refused));
        Assert.Equal("key not stored", outcomes.Read(refused, Words));
        Assert.False(outcomes.IsNew(refused));
        Assert.Null(outcomes.Read(refused, Words));
    }

    // Settings opened again starts past what the form already carries.
    [Fact]
    public void AnOutcomeFromBeforeSettingsOpenedIsNotSaid()
    {
        var earlier = Outcome(4, new AiEndpointResult.KeyNotStored());
        var outcomes = new AiEndpointOutcomes(Form(earlier));
        Assert.Null(outcomes.Read(earlier, Words));
        Assert.Equal("key not stored", outcomes.Read(Outcome(5, new AiEndpointResult.KeyNotStored()), Words));
    }

    // A change that worked shows its result in the fields; a refusal is said beside them; what
    // could not be written is the failed-save notice's.
    [Theory]
    [InlineData("saved")]
    [InlineData("removed")]
    [InlineData("refused")]
    [InlineData("not written")]
    public void OnlyAKeyNotStoredIsSaidOnTheForm(string came)
    {
        AiEndpointResult result = came switch
        {
            "saved" => new AiEndpointResult.Saved(),
            "removed" => new AiEndpointResult.Removed(),
            "refused" => new AiEndpointResult.Refused(None with { ModelMissing = true }),
            _ => new AiEndpointResult.NotWritten(),
        };
        var outcomes = new AiEndpointOutcomes(Form());
        Assert.Null(outcomes.Read(Outcome(1, result), Words));
        Assert.False(outcomes.IsNew(Outcome(1, result)));
    }

    [Fact]
    public void TheFieldsTheRecordDrawsLeaveTheKeyEmpty()
    {
        var form = Form() with { Address = "https://a.example.eu", Model = "m", Declared = JurisdictionClass.EuHosted, KeyKept = true };
        Assert.Equal(new AiEndpointTyped("https://a.example.eu", string.Empty, "m", JurisdictionClass.EuHosted), AiEndpointTyped.Of(form));
    }
}
