// Bridges the Rust core's log records (delivered over the FFI Logger port) into the
// platform file Log, so every layer, the shared Rust core, the binding layer, and this
// WinUI host, lands in one app.log. That single stream is what makes a field issue (a slow
// boot, an empty calendar) diagnosable after the fact. The Windows counterpart of macOS's
// os_log forwarder and Android's android.util.Log forwarder. The core gates records by level
// before they cross the FFI, so this just maps the level and forwards.

using uniffi.mailcal_bindings;
// Disambiguate the file logger from this class's own `Log` interface method.
using FileLog = Allodia.Mailcal.Services.Log;

namespace Allodia.Mailcal.Services;

/// <summary>Forwards Rust-core log records to the file <see cref="Log"/>, mapping the FFI
/// <see cref="LogLevel"/> to the file log's levels.</summary>
internal sealed class CoreLogger : Logger
{
    public void Log(LogLevel @level, string @target, string @message)
    {
        // Prefix with the emitting module (e.g. mailcal_app::sync) so core lines are
        // distinguishable from this host's own Log.Info calls in the shared file.
        var line = $"[{@target}] {@message}";
        switch (@level)
        {
            case LogLevel.Error:
                FileLog.Error(line);
                break;
            case LogLevel.Warn:
                FileLog.Warn(line);
                break;
            case LogLevel.Info:
                FileLog.Info(line);
                break;
            case LogLevel.Debug:
                FileLog.Debug(line);
                break;
            default:
                FileLog.Trace(line);
                break;
        }
    }
}
