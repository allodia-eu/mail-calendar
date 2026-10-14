// A release build has no web inspector (docs/rendering-security.md, gate 2a). The variables the
// WebView2 loader reads are taken out of the process before the first web view, because each one
// reaches past the app's own options: a remote-debugging port in the browser's arguments hands every
// page to any process on the machine.

using System;
using System.Collections;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public sealed class WebViewInspectionTests
{
    [Fact]
    public void EveryLoaderVariableIsNamedWhateverItsCaseAndNothingElse()
    {
        var environment = new Hashtable
        {
            ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = "--remote-debugging-port=9222",
            ["webview2_browser_executable_folder"] = @"C:\elsewhere",
            ["WEBVIEW2_PIPE_FOR_SCRIPT_DEBUGGER"] = "pipe",
            ["WEBVIEW"] = "not the loader's",
            ["PATH"] = @"C:\Windows",
        };

        Assert.Equal(
            ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "webview2_browser_executable_folder", "WEBVIEW2_PIPE_FOR_SCRIPT_DEBUGGER"],
            WebViewInspection.LoaderVariables(environment));
    }

    [Fact]
    public void ABuildThatMayNotBeInspectedTakesTheLoaderVariablesOutOfTheProcess()
    {
        const string Port = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";
        const string Unrelated = "MAILCAL_TEST_UNRELATED";
        Environment.SetEnvironmentVariable(Port, "--remote-debugging-port=9222");
        Environment.SetEnvironmentVariable(Unrelated, "kept");
        try
        {
            Assert.True(WebViewInspection.Close(inspectable: false) >= 1);
            Assert.Null(Environment.GetEnvironmentVariable(Port));
            Assert.Equal("kept", Environment.GetEnvironmentVariable(Unrelated));
        }
        finally
        {
            Environment.SetEnvironmentVariable(Port, null);
            Environment.SetEnvironmentVariable(Unrelated, null);
        }
    }

    [Fact]
    public void ABuildThatMayBeInspectedLeavesThemForTheDevScripts()
    {
        const string Port = "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS";
        Environment.SetEnvironmentVariable(Port, "--remote-debugging-port=9222");
        try
        {
            Assert.Equal(0, WebViewInspection.Close(inspectable: true));
            Assert.Equal("--remote-debugging-port=9222", Environment.GetEnvironmentVariable(Port));
        }
        finally
        {
            Environment.SetEnvironmentVariable(Port, null);
        }
    }
}
