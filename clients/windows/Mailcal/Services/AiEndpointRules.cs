// What Settings → Advanced's own endpoint form does with the core's record (docs/ai.md rules 6 and
// 7): the words beside each field for what a save would refuse, Save as drawn, the key sent with a
// save, and which outcome is said, once. WinUI-free and L10n-free, so Mailcal.Tests can link it;
// the words arrive as parameters and the form itself (Dialogs/SettingsDialog.AiEndpoint.cs) draws
// the answer.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>The catalog's words for what the form says.</summary>
/// <param name="AddressMissing"><c>ai_endpoint_gap_address_missing</c>.</param>
/// <param name="AddressUnusable"><c>ai_endpoint_gap_address_unusable</c>.</param>
/// <param name="NeedsHttps"><c>ai_endpoint_gap_needs_https</c>.</param>
/// <param name="ModelMissing"><c>ai_endpoint_gap_model_missing</c>.</param>
/// <param name="WhereMissing"><c>ai_endpoint_gap_where_missing</c>.</param>
/// <param name="RefusedEuNative"><c>ai_endpoint_gap_refused_eu_native</c>.</param>
/// <param name="RefusedEuHosted"><c>ai_endpoint_gap_refused_eu_hosted</c>.</param>
/// <param name="KeyHint"><c>ai_endpoint_key_hint</c>.</param>
/// <param name="KeyKept"><c>ai_endpoint_key_stored</c>.</param>
/// <param name="KeyNotStored"><c>ai_endpoint_key_not_stored</c>.</param>
internal sealed record AiEndpointWords(
    string AddressMissing,
    string AddressUnusable,
    string NeedsHttps,
    string ModelMissing,
    string WhereMissing,
    string RefusedEuNative,
    string RefusedEuHosted,
    string KeyHint,
    string KeyKept,
    string KeyNotStored);

/// <summary>What the form says beside each field; null where the field has nothing to say.</summary>
internal sealed record AiEndpointGapLines(string? Address, string? Model, string? Where);

/// <summary>The form's fields as typed.</summary>
internal sealed record AiEndpointTyped(string Address, string Key, string Model, JurisdictionClass? Declared)
{
    /// <summary>The fields as the record draws them, the key field empty: it never draws one back.</summary>
    internal static AiEndpointTyped Of(AiEndpointForm form) => new(form.Address, string.Empty, form.Model, form.Declared);

    /// <summary>Whether nothing has been typed or chosen, as on a form with nothing set up.</summary>
    internal bool Blank =>
        Address.Trim().Length == 0 && Key.Trim().Length == 0 && Model.Trim().Length == 0 && Declared is null;
}

internal static class AiEndpointRules
{
    /// <summary>
    /// What the form says beside each field for <paramref name="gaps"/>, the core's answer for the
    /// fields as typed. A form left blank says nothing: Save is held, and a page of demands under
    /// fields nobody has touched reads as an error.
    /// </summary>
    internal static AiEndpointGapLines Lines(AiEndpointGaps gaps, AiEndpointTyped typed, AiEndpointWords words)
    {
        if (typed.Blank)
        {
            return new AiEndpointGapLines(null, null, null);
        }
        var address = gaps.Address switch
        {
            AiEndpointAddressGap.Missing => words.AddressMissing,
            AiEndpointAddressGap.Unusable => words.AddressUnusable,
            AiEndpointAddressGap.NeedsHttps => words.NeedsHttps,
            _ => null,
        };
        var where = gaps.Declaration switch
        {
            AiEndpointDeclarationGap.Missing => words.WhereMissing,
            AiEndpointDeclarationGap.Refused refused => Refused(refused.Mode, words),
            _ => null,
        };
        return new AiEndpointGapLines(address, gaps.ModelMissing ? words.ModelMissing : null, where);
    }

    /// <summary>Whether a save of the fields would go ahead.</summary>
    internal static bool Complete(AiEndpointGaps gaps) =>
        gaps.Address is null && !gaps.ModelMissing && gaps.Declaration is null;

    /// <summary>Save as drawn: the record's, and held besides while the fields as typed leave
    /// something out (<c>ai_endpoint_gap</c>).</summary>
    internal static ControlState SaveState(ControlState save, AiEndpointGaps typed) =>
        new(save.Enabled && Complete(typed), save.Running);

    /// <summary>The key a save sends: what was typed, or <c>null</c> for a field left empty, which
    /// keeps the key the form says is kept and stores none where none is. Spaces alone are an empty
    /// field: sent, the core would trim them to nothing and take the kept key out.</summary>
    internal static string? Key(string typed) => typed.Trim().Length == 0 ? null : typed;

    /// <summary>The line under the key field.</summary>
    internal static string KeyNote(bool kept, AiEndpointWords words) => kept ? words.KeyKept : words.KeyHint;

    // Only a mode narrower than "anywhere" refuses a declaration, and each says what it admits.
    private static string Refused(JurisdictionMode mode, AiEndpointWords words) =>
        mode == JurisdictionMode.EuHosted ? words.RefusedEuHosted : words.RefusedEuNative;
}

/// <summary>
/// The outcomes the form has already seen, from the moment Settings opened, so each newer one is
/// said once: a page drawn again, or Settings opened again, does not say an old one twice.
/// </summary>
internal sealed class AiEndpointOutcomes
{
    private ulong _seen;

    /// <summary>Starts past whatever <paramref name="form"/> already carries.</summary>
    internal AiEndpointOutcomes(AiEndpointForm? form) => _seen = form?.Outcome?.Serial ?? 0;

    /// <summary>
    /// What to say for <paramref name="outcome"/>, or <c>null</c>: nothing new, or nothing the form
    /// says. A save or a removal that worked shows its result in the fields; a refusal is said
    /// beside the fields; preferences that could not be written are the failed-save notice's.
    /// Only a key the keystore refused is said on the form.
    /// </summary>
    internal string? Read(AiEndpointOutcome? outcome, AiEndpointWords words)
    {
        if (outcome is null || outcome.Serial <= _seen)
        {
            return null;
        }
        _seen = outcome.Serial;
        return outcome.Result is AiEndpointResult.KeyNotStored ? words.KeyNotStored : null;
    }

    /// <summary>Whether <paramref name="outcome"/> is one this form has not acted on yet.</summary>
    internal bool IsNew(AiEndpointOutcome? outcome) => outcome is not null && outcome.Serial > _seen;
}
