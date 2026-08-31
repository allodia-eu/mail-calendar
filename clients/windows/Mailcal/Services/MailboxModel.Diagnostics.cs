// The Settings → Diagnostics half of MailboxModel (its own small partial: Accounts.cs is near
// the 500-line limit, and the generated UniFFI types stay confined to Services): the debug
// log-level toggle, which persists the choice (LogLevelStore) and raises the ceiling on the
// live core in one step, no restart. The boot path applies the same persisted choice via
// ResolveLogLevel (MailboxModel.Accounts.cs), where the ALLODIA_LOG_LEVEL env var still wins
// as the dev escape hatch.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>Whether the persisted Diagnostics choice is debug (seeds the settings toggle).</summary>
    public bool DiagnosticsDebugEnabled => LogLevelStore.Read() == "debug";

    /// <summary>
    /// Applies the Settings → Diagnostics debug toggle: persists the choice so the next boot
    /// resolves it, and raises/lowers the live core's log ceiling at once. The level change is
    /// itself logged, a level name is an event, never content, so it's privacy-safe.
    /// </summary>
    public void SetDiagnosticsDebug(bool on)
    {
        var choice = on ? "debug" : "info";
        LogLevelStore.Write(choice);
        _app?.SetLogLevel(on ? LogLevel.Debug : LogLevel.Info);
        Log.Info($"diagnostics: log level set to {choice} from settings");
    }
}
