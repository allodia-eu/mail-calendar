// Settings → Advanced as the core decides it (AdvancedPage): AI assistant access first, the
// setting a person comes to this category for (SettingsDialog.Mcp.cs), then the reset of the local
// database. Both follow Settings signals while the category is open, because the server binds or
// is refused and a reset starts and ends while the person watches.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The reset's group, its content, and what that content was last drawn from.
    private UIElement? _resetGroup;
    private StackPanel? _reset;
    private string? _resetDrawn;

    // Whether the question before a reset is open, so a redraw while it is keeps it open.
    private bool _resetAsking;

    private UIElement BuildAdvanced()
    {
        var panel = new StackPanel { Spacing = 20 };
        panel.Children.Add(AssistantsHost());
        _reset = new StackPanel { Spacing = 8 };
        _resetDrawn = null;
        _resetAsking = false;
        _resetGroup = Group(L10n.ActionResetDatabase(), L10n.SettingsAdvancedResetDescription(), _reset);
        panel.Children.Add(_resetGroup);
        RefreshAdvanced();
        return panel;
    }

    private void RefreshAdvanced()
    {
        var page = _model.Advanced;
        RefreshAssistants(page?.Assistants);
        RefreshReset(page);
    }

    // The button, what the last reset came to beneath it, and the question it opens. Drawn again
    // only when the core's record of the reset moved.
    private void RefreshReset(AdvancedPage? page)
    {
        if (_reset is not { } host || _resetGroup is null)
        {
            return;
        }
        _resetGroup.Visibility = page is null ? Visibility.Collapsed : Visibility.Visible;
        var drawn = page is null ? string.Empty : AdvancedPageRules.ResetFingerprint(page);
        if (drawn == _resetDrawn)
        {
            return;
        }
        _resetDrawn = drawn;
        host.Children.Clear();
        if (page is null)
        {
            return;
        }
        var confirm = new ConfirmPanel(
            L10n.ResetTitle(),
            L10n.ResetMessage(),
            L10n.ResetConfirm(),
            "ResetDatabaseAsk",
            yes =>
            {
                _resetAsking = false;
                if (yes)
                {
                    _model.ResetDatabase();
                }
            });
        host.Children.Add(ControlComponents.Button(
            L10n.ActionResetDatabase(), page.Reset, "ResetDatabase", () =>
            {
                _resetAsking = true;
                confirm.Show();
            }));
        if (AdvancedPageRules.OutcomeText(page.ResetOutcome, ResetWords()) is { } said)
        {
            var line = page.ResetOutcome == ResetOutcome.Failed ? CriticalLine(said) : Description(said);
            line.Opacity = 1;
            AutomationProperties.SetAutomationId(line, "ResetDatabaseOutcome");
            host.Children.Add(line);
        }
        confirm.Enabled = page.Reset.Enabled;
        if (_resetAsking)
        {
            confirm.Show();
        }
        host.Children.Add(confirm.Element);
    }

    private static ResetOutcomeWords ResetWords() =>
        new(L10n.SettingsAdvancedResetDone(), L10n.SettingsAdvancedResetFailed());
}
