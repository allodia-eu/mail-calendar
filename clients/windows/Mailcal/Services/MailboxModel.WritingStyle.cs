// The Writing style surface of the model (docs/ai.md): the learned library, each account's style,
// the own AI endpoint, and the calls that wait on the Sent folder or on the AI endpoint. Those go
// off the UI thread here, as the Allodia account's calls do. Its own partial to keep MailboxModel.cs
// under the 500-line limit.
//
// Like the signature library, nothing is cached: the snapshot is small and local, and an assignment
// or the route can change under an open screen.

using System.Threading.Tasks;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>What a call that waits answered: its value, or why there is none.</summary>
/// <param name="Value">The answer, or <c>null</c> on failure.</param>
/// <param name="Failure">Why there is no answer, or <c>null</c> on success.</param>
internal sealed record AiOutcome<T>(T? Value, AiFailure? Failure)
    where T : class;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Raised on the UI thread when the core signals the Writing style surface: the library, an
    /// assignment, the route, the balance or a learning run's progress changed.
    /// </summary>
    internal event Action? WritingStylesChanged;

    /// <summary>
    /// The Writing style surface. <c>Route</c> is <c>null</c> when AI has nowhere to go, which is
    /// also the answer before the core is up.
    /// </summary>
    internal WritingStyleSnapshot WritingStyles =>
        _app?.WritingStyles()
        ?? new WritingStyleSnapshot(
            null, null, Array.Empty<WritingStyleRow>(), Array.Empty<AccountWritingStyleRow>(), null, null);

    /// <summary>Passes a <c>Surface.WritingStyle</c> signal on to whatever is drawing it.</summary>
    private void OnWritingStyleSignal() => WritingStylesChanged?.Invoke();

    /// <summary>One style in full, or <c>null</c> when the id names nothing.</summary>
    internal WritingStyleDetail? WritingStyleDetailOf(string id) => _app?.WritingStyleDetail(id);

    /// <summary>Renames a style.</summary>
    internal void RenameWritingStyle(string id, string name) => _app?.RenameWritingStyle(id, name);

    /// <summary>Replaces a style's notes.</summary>
    internal void UpdateWritingStyleNotes(string id, string notes) => _app?.UpdateWritingStyleNotes(id, notes);

    /// <summary>Forgets a style. The core clears it from every account that drafted in it.</summary>
    internal void DeleteWritingStyle(string id) => _app?.DeleteWritingStyle(id);

    /// <summary>Assigns a style to an account, or clears it with <c>null</c>.</summary>
    internal void SetAccountWritingStyle(string account, string? style) =>
        _app?.SetAccountWritingStyle(account, style);

    /// <summary>The style <paramref name="account"/> drafts in, or <c>null</c>.</summary>
    internal string? ResolveWritingStyle(string account) => _app?.ResolveWritingStyle(account);

    /// <summary>Stops a learning run before its next request.</summary>
    internal void CancelWritingStyleLearning() => _app?.CancelWritingStyleLearning();

    /// <summary>
    /// What learning from <paramref name="account"/>'s sent mail up to <paramref name="until"/>
    /// would read and send. Reads the Sent folder, so it runs off the UI thread; nothing leaves
    /// the device.
    /// </summary>
    internal Task<AiOutcome<CorpusReport>> SentCorpusReportAsync(string account, long? until) =>
        OffUiThread("the sent mail could not be read", () => _app!.SentCorpusReport(account, null, until));

    /// <summary>
    /// Learns a style from <paramref name="account"/>'s sent mail. Pressing Learn on the consent
    /// sheet is what calls this. Progress arrives as <see cref="WritingStylesChanged"/>.
    /// </summary>
    internal Task<AiOutcome<LearnReport>> LearnWritingStyleAsync(
        string account, long? until, string name, string uiLanguage) =>
        OffUiThread(
            "no style was learned",
            () => _app!.LearnWritingStyle(account, null, until, name, uiLanguage));

    /// <summary>
    /// Drafts a reply to <paramref name="key"/> in <paramref name="account"/>, in the style of
    /// <paramref name="from"/> when given, else of the account holding the message. Nothing is
    /// sent: the composer puts the text in front of the person.
    /// </summary>
    internal Task<AiOutcome<DraftReply>> DraftReplyAsync(
        string account, string key, string? from, string? intent) =>
        OffUiThread(
            "no reply was drafted",
            () => _app!.DraftReply(account, key, from, null, intent, null));

    /// <summary>
    /// Asks Allodia's relay for the credits left. A failure changes nothing: the line keeps the
    /// balance last reported.
    /// </summary>
    internal async Task RefreshAiBalanceAsync()
    {
        if (_app is null)
        {
            return;
        }
        try
        {
            await Task.Run(() => _app!.RefreshAiBalance());
        }
        catch (Exception ex)
        {
            Log.Info($"ai: the credits were not refreshed ({AiFailure.For(ex, AiRoute.Relay).Problem})");
        }
    }

    /// <summary>The own endpoint's settings, or <c>null</c> when none is set up.</summary>
    internal OwnAiEndpoint? OwnEndpoint => _app?.OwnAiEndpoint();

    /// <summary>
    /// Saves the own endpoint and switches AI to it. Returns why it was not saved, or <c>null</c>.
    /// A <c>null</c> <paramref name="key"/> keeps the stored one.
    /// </summary>
    internal OwnEndpointProblem? SaveOwnEndpoint(
        string baseUrl, string model, JurisdictionClass? declared, string? key)
    {
        if (_app is null)
        {
            return OwnEndpointProblem.Keystore;
        }
        try
        {
            _app.SetOwnAiEndpoint(baseUrl, model, declared, key);
            return null;
        }
        catch (Exception ex)
        {
            var problem = OwnEndpointForm.ProblemFor(ex);
            Log.Info($"ai: the own endpoint was not saved ({problem})");
            return problem;
        }
    }

    /// <summary>
    /// Removes the own endpoint and its key. Returns <see cref="OwnEndpointProblem.Keystore"/>
    /// when the key could not be deleted; the settings are removed either way.
    /// </summary>
    internal OwnEndpointProblem? RemoveOwnEndpoint()
    {
        try
        {
            _app?.ClearOwnAiEndpoint();
            return null;
        }
        catch (Exception ex)
        {
            var problem = OwnEndpointForm.ProblemFor(ex);
            Log.Info($"ai: the own endpoint's key was not deleted ({problem})");
            return problem;
        }
    }

    // One blocking core call on a pool thread, its failure worded for the route in force when it
    // was asked. `what` is the log line's own words: a failure's kind is logged, never its text.
    private async Task<AiOutcome<T>> OffUiThread<T>(string what, Func<T> call)
        where T : class
    {
        if (_app is null)
        {
            return new AiOutcome<T>(null, new AiFailure(AiProblem.Unavailable));
        }
        var route = WritingStyles.Route;
        try
        {
            return new AiOutcome<T>(await Task.Run(call), null);
        }
        catch (Exception ex)
        {
            var failure = AiFailure.For(ex, route);
            Log.Info($"writing style: {what} ({failure.Problem})");
            return new AiOutcome<T>(null, failure);
        }
    }
}
