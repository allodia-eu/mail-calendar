// Asking the Microsoft Store whether it is holding a newer version of this app, and letting it
// install one.
//
// The twin of AppIdentity.CheckForUpdateAsync, which answers the same question for a copy installed
// from an `.appinstaller` (docs/updates.md). Neither can answer for the other's channel: this one
// asks the Store's licensing service about a Store product, and `Package.CheckUpdateAvailabilityAsync`
// asks App Installer about a URL the Store never gave us.
//
// Three constraints come from the API rather than from us, and each one shapes what is written here.

using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using Windows.Services.Store;
using WinRT.Interop;

namespace Allodia.Mailcal.Services;

/// <summary>The Microsoft Store's answer about this app's own package.</summary>
internal sealed class StoreUpdates
{
    private readonly IntPtr owner;

    /// <summary>Built on first use, inside the same guard as the call that needs it.</summary>
    private StoreContext? context;

    /// <summary>What the last check found, so the install acts on what the user was shown.</summary>
    private IReadOnlyList<StorePackageUpdate> pending = Array.Empty<StorePackageUpdate>();

    /// <param name="owner">
    /// The window these calls hang their modal dialogs off. Without it a desktop app gets
    /// <c>ERROR_INVALID_WINDOW_HANDLE</c> from a method that otherwise looks like it should just
    /// work, which is the documented failure and not an obvious one to read.
    /// </param>
    internal StoreUpdates(IntPtr owner) => this.owner = owner;

    /// <summary>The Store's own handle on this app, built once.</summary>
    /// <remarks>
    /// Resolved here rather than in the constructor so that a machine whose Store is missing or
    /// broken costs an answer on a button press. Built eagerly it would throw out of the click
    /// handler that made it, which is an unhandled exception in Settings rather than a line of
    /// status text.
    /// </remarks>
    private StoreContext Context()
    {
        if (context is null)
        {
            var resolved = StoreContext.GetDefault();
            InitializeWithWindow.Initialize(resolved, owner);
            context = resolved;
        }
        return context;
    }

    /// <summary>Whether the Store has a newer version of this app.</summary>
    /// <remarks>
    /// Must be awaited from the UI thread; off it, the call fails the same way a missing owner
    /// window does.
    ///
    /// ⚠️ **The answer can be a cached one, and that is the API's design rather than a fault.** It
    /// performs at most one real check every 30 minutes, and ten in a day; past either, it returns
    /// what it last found. So a person pressing this twice gets the same answer twice even if the
    /// Store's own state moved in between, and the copy says what is true of a check ("this is the
    /// latest version") rather than claiming a fresh one was made.
    /// </remarks>
    internal async Task<UpdateOutcome> CheckAsync()
    {
        try
        {
            pending = await Context().GetAppAndOptionalStorePackageUpdatesAsync();
            return pending.Count == 0 ? UpdateOutcome.UpToDate : UpdateOutcome.Available;
        }
        catch (Exception problem)
        {
            // This needs the Store app itself installed and its services running, which is exactly
            // what the machines the other channel exists for do not have. A build that finds itself
            // there says it could not check, which is true, rather than taking Settings down.
            Log.Warn($"store update check failed: {problem.GetType().Name}");
            pending = Array.Empty<StorePackageUpdate>();
            return UpdateOutcome.Failed;
        }
    }

    /// <summary>Hand the update the last check found to the Store to download and install.</summary>
    /// <remarks>
    /// The Store shows its own progress and consent UI, which is why this needs the owner window.
    /// It acts on the list the check returned rather than asking again, so the user installs what
    /// they were told about.
    /// </remarks>
    internal async Task InstallAsync()
    {
        if (pending.Count == 0)
        {
            return;
        }
        try
        {
            await Context().RequestDownloadAndInstallStorePackageUpdatesAsync(pending);
        }
        catch (Exception problem)
        {
            Log.Warn($"store update install failed: {problem.GetType().Name}");
        }
    }
}
