// The own endpoint's form under Settings → Advanced (docs/ai.md rules 6 and 7), as the core draws
// it. Nothing is cached here: the form is read from the core's memory each time.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>Raised when the own endpoint's form may have changed (<c>Surface.AiEndpoint</c>, and
    /// <c>Surface.Settings</c>): Settings → Advanced reads <see cref="AiEndpointForm"/> again.</summary>
    internal event Action? AiEndpointChanged;

    /// <summary>The form, or <c>null</c> while the writing assistant is not offered, when nothing
    /// is drawn. Read from memory, so the UI thread may ask it.</summary>
    internal AiEndpointForm? AiEndpointForm() => _app?.AiEndpointForm();

    /// <summary>What a save of these fields would refuse, the same answer the save acts on. Read
    /// from memory, so it is asked as the person types. Everything is missing before the app has
    /// connected, which holds Save.</summary>
    internal AiEndpointGaps AiEndpointGap(string address, string model, JurisdictionClass? declared) =>
        _app?.AiEndpointGap(address, model, declared)
        ?? new AiEndpointGaps(AiEndpointAddressGap.Missing, true, new AiEndpointDeclarationGap.Missing());

    /// <summary>A save or a removal. Returns at once: the change runs in the core, and what it came
    /// to is the form's outcome.</summary>
    internal void ChangeAiEndpoint(AiEndpointIntent intent) =>
        _app?.Dispatch(new Intent.Ai(new AiIntent.Endpoint(intent)));

    private void RaiseAiEndpointChanged() => AiEndpointChanged?.Invoke();
}
