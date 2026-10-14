// A release build has no web inspector (docs/rendering-security.md, gate 2a): not the WebView2
// developer tools, and not the remote-debugging channel an environment variable opens.
//
// The WebView2 loader reads a family of WEBVIEW2_* variables from the host process when it creates
// an environment, and several of them reach past the app's own choices:
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS appends to the browser's command line (a
// `--remote-debugging-port` there hands every page, a message being read included, to any process
// on this machine that connects), WEBVIEW2_BROWSER_EXECUTABLE_FOLDER swaps the browser runtime, and
// WEBVIEW2_PIPE_FOR_SCRIPT_DEBUGGER announces each page on a named pipe. Removing them from this
// process before the first web view is created is the only control the app has: the app's own
// options are appended to, or replaced by, what the variables say.
//
// A debug build keeps both: the developer tools, and the variables the dev scripts use to reach the
// composer's page (clients/windows/composer.ps1).

using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;

namespace Allodia.Mailcal.Services;

/// <summary>Whether this build's web views can be inspected, and the environment that would open them.</summary>
public static class WebViewInspection
{
    /// <summary>The prefix of every variable the WebView2 loader reads.</summary>
    public const string LoaderPrefix = "WEBVIEW2_";

    /// <summary>Whether a web view offers its developer tools: in a debug build only.</summary>
#if DEBUG
    public const bool DevToolsEnabled = true;
#else
    public const bool DevToolsEnabled = false;
#endif

    /// <summary>
    /// The names in <paramref name="environment"/> the WebView2 loader would read. Matched without
    /// regard to case, because Windows does not distinguish case in a variable's name.
    /// </summary>
    public static IReadOnlyList<string> LoaderVariables(IDictionary environment) =>
        environment.Keys.OfType<string>()
            .Where(name => name.StartsWith(LoaderPrefix, StringComparison.OrdinalIgnoreCase))
            .OrderBy(name => name, StringComparer.OrdinalIgnoreCase)
            .ToList();

    /// <summary>
    /// Unless <paramref name="inspectable"/>, removes every loader variable from this process and
    /// returns how many there were. Called once, before anything creates a web view.
    /// </summary>
    public static int Close(bool inspectable)
    {
        if (inspectable)
        {
            return 0;
        }
        var names = LoaderVariables(Environment.GetEnvironmentVariables());
        foreach (var name in names)
        {
            Environment.SetEnvironmentVariable(name, null);
        }
        return names.Count;
    }
}
