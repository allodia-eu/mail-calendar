// Settings → Diagnostics: the local diagnostic log surfaced to the user, status rows (total
// size across app.log + backups, backup count, the ~4 MB cap note), an inline read-only viewer
// of the current file (monospace, newest last, opens scrolled to the end), export to a
// user-picked file behind an inline privacy-note confirm, a copy-the-path shortcut, and the
// debug-verbosity toggle for a support session. The Windows leg of the cross-platform contract
// in docs/logging.md; split into its own partial to keep SettingsDialog.cs under the 500-line
// limit.
//
// The log is privacy-safe by construction (counts, ids, durations, events, never message
// content, addresses, or credentials; see Services/Log.cs and docs/logging.md), which is what
// makes showing and exporting it here safe. All the pure logic (snapshot math, tail read,
// export payload read, byte formatting) lives in Services/DiagnosticsLog.cs, unit-tested; this
// file only draws it. The dialog is 680x500 and a nested ContentDialog isn't allowed, so both
// the viewer and the export confirm expand inline (the BuildAdvanced pattern).

using System.Globalization;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private UIElement BuildDiagnostics()
    {
        var panel = new StackPanel { Spacing = 20 };
        panel.Children.Add(Group(
            L10n.DiagnosticsLogHeading(), L10n.DiagnosticsLogDescription(), DiagnosticsLogControls()));
        panel.Children.Add(Group(
            L10n.DiagnosticsDebugHeading(), L10n.DiagnosticsDebugDescription(), DiagnosticsDebugToggle()));
        return panel;
    }

    // The log group's content: status rows, the action row, the transient copy feedback, the
    // inline export confirm, and the (initially collapsed) viewer with its jump-to-end button.
    private UIElement DiagnosticsLogControls()
    {
        var stack = new StackPanel { Spacing = 8 };

        var (totalBytes, backupCount) = Log.Snapshot();
        stack.Children.Add(StatusRow(
            L10n.DiagnosticsLogSizeLabel(), DiagnosticsLog.FormatBytes(totalBytes)));
        stack.Children.Add(StatusRow(
            L10n.DiagnosticsLogBackupsLabel(), backupCount.ToString(CultureInfo.CurrentCulture)));
        stack.Children.Add(new TextBlock
        {
            Text = L10n.DiagnosticsLogCapNote(),
            TextWrapping = TextWrapping.Wrap,
            Opacity = 0.7,
        });

        // The inline viewer: read-only, monospace, newest last. Collapsed until "View log".
        var viewerText = new TextBlock
        {
            FontFamily = new FontFamily("Consolas"),
            FontSize = 12,
            TextWrapping = TextWrapping.Wrap,
            IsTextSelectionEnabled = true,
        };
        var viewer = new ScrollViewer
        {
            Content = viewerText,
            Height = 220,
            Visibility = Visibility.Collapsed,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
        };
        AutomationProperties.SetAutomationId(viewer, "DiagnosticsLogViewer");
        var jump = new Button
        {
            Content = L10n.DiagnosticsJumpToEnd(),
            Visibility = Visibility.Collapsed,
        };
        AutomationProperties.SetAutomationId(jump, "DiagnosticsJumpToEnd");
        void JumpToEnd()
        {
            // The freshly set text must be measured before the scroll extent exists.
            viewer.UpdateLayout();
            _ = viewer.ChangeView(null, viewer.ScrollableHeight, null, disableAnimation: true);
        }
        jump.Click += (_, _) => JumpToEnd();
        // Like the Android/Apple viewers: the affordance shows only while scrolled away from the
        // end (at the end there is nothing left to jump to, so it disappears rather than lying).
        viewer.ViewChanged += (_, _) => jump.Visibility =
            viewer.VerticalOffset < viewer.ScrollableHeight - 1
                ? Visibility.Visible
                : Visibility.Collapsed;

        var view = new Button { Content = L10n.DiagnosticsViewLog() };
        AutomationProperties.SetAutomationId(view, "DiagnosticsViewLog");
        view.Click += (_, _) =>
        {
            // (Re)load on every click, so a second press refreshes a viewer that's already open.
            var text = Log.ReadCurrent();
            viewerText.Text = text.Length == 0 ? L10n.DiagnosticsLogEmpty() : text;
            viewer.Visibility = Visibility.Visible;
            // Open scrolled to the end (newest last) on the next dispatcher turn, once the new
            // text has laid out; ViewChanged then keeps the jump affordance honest.
            _ = DispatcherQueue.TryEnqueue(JumpToEnd);
        };

        // Export, behind the inline privacy-note confirm (the BuildAdvanced pattern, a nested
        // ContentDialog isn't allowed, so the button reveals the note + confirm/cancel in place).
        var export = new Button { Content = L10n.DiagnosticsExportLog() };
        AutomationProperties.SetAutomationId(export, "DiagnosticsExportLog");
        var confirm = new StackPanel { Spacing = 8, Visibility = Visibility.Collapsed };
        confirm.Children.Add(Heading(L10n.DiagnosticsShareConfirmTitle()));
        confirm.Children.Add(new TextBlock
        {
            Text = L10n.DiagnosticsSharePrivacyNote(),
            TextWrapping = TextWrapping.Wrap,
        });
        var confirmButtons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var confirmExport = new Button { Content = L10n.DiagnosticsExportLog() };
        AutomationProperties.SetAutomationId(confirmExport, "DiagnosticsExportConfirm");
        confirmExport.Click += async (_, _) =>
        {
            confirm.Visibility = Visibility.Collapsed;
            export.Visibility = Visibility.Visible;
            await ExportLogAsync();
        };
        var cancel = new Button { Content = L10n.ActionCancel() };
        cancel.Click += (_, _) =>
        {
            confirm.Visibility = Visibility.Collapsed;
            export.Visibility = Visibility.Visible;
        };
        confirmButtons.Children.Add(confirmExport);
        confirmButtons.Children.Add(cancel);
        confirm.Children.Add(confirmButtons);
        export.Click += (_, _) =>
        {
            export.Visibility = Visibility.Collapsed;
            confirm.Visibility = Visibility.Visible;
        };

        // Copy the absolute log path, with a transient "copied" confirmation.
        var copied = new TextBlock
        {
            Text = L10n.DiagnosticsPathCopied(),
            Opacity = 0.7,
            Visibility = Visibility.Collapsed,
        };
        var copy = new Button { Content = L10n.DiagnosticsCopyPath() };
        AutomationProperties.SetAutomationId(copy, "DiagnosticsCopyPath");
        copy.Click += (_, _) =>
        {
            if (Log.FilePath is not { } path)
            {
                return;
            }
            var package = new DataPackage();
            package.SetText(path);
            Clipboard.SetContent(package);
            copied.Visibility = Visibility.Visible;
            // Transient: hide the confirmation again after a moment (one-shot timer).
            var timer = DispatcherQueue.CreateTimer();
            timer.Interval = TimeSpan.FromSeconds(2);
            timer.IsRepeating = false;
            timer.Tick += (_, _) => copied.Visibility = Visibility.Collapsed;
            timer.Start();
        };

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        actions.Children.Add(view);
        actions.Children.Add(export);
        actions.Children.Add(copy);
        stack.Children.Add(actions);
        stack.Children.Add(copied);
        stack.Children.Add(confirm);
        stack.Children.Add(jump);
        stack.Children.Add(viewer);
        return stack;
    }

    // The debug-verbosity toggle: ON raises the live core to Debug and persists the choice for
    // the next boot; OFF restores Info. IsOn is set before the handler attaches, so seeding the
    // stored choice doesn't re-fire the model (the Radio pattern).
    private UIElement DiagnosticsDebugToggle()
    {
        var toggle = new ToggleSwitch { IsOn = _model.DiagnosticsDebugEnabled };
        AutomationProperties.SetAutomationId(toggle, "DiagnosticsDebugToggle");
        AutomationProperties.SetName(toggle, L10n.DiagnosticsDebugHeading());
        toggle.Toggled += (_, _) =>
        {
            if (!_rebuilding)
            {
                _model.SetDiagnosticsDebug(toggle.IsOn);
            }
        };
        return toggle;
    }

    // A "label value" status row (log size, backup count).
    private static UIElement StatusRow(string label, string value)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        row.Children.Add(new TextBlock { Text = label, Opacity = 0.7 });
        row.Children.Add(new TextBlock { Text = value });
        return row;
    }

    // Exports the current log file to a user-picked path, the FileSavePicker flow
    // ReadingView.Attachments.cs uses, with the same delete-on-failure so a failed copy never
    // leaves a half-written file at the chosen path. The live file only, exactly what the
    // Android/Apple share hands over (docs/logging.md); backups stay on the device.
    private static async Task ExportLogAsync()
    {
        StorageFile? file;
        try
        {
            var picker = new FileSavePicker { SuggestedFileName = "allodia-mail-diagnostics" };
            picker.FileTypeChoices.Add(L10n.DiagnosticsLogHeading(), new List<string> { ".log" });
            if (App.MainWindow is not null)
            {
                InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(App.MainWindow));
            }
            file = await picker.PickSaveFileAsync();
        }
        catch (Exception ex)
        {
            Log.Warn($"diagnostics export picker failed: {ex.GetType().Name}");
            return;
        }
        if (file is null)
        {
            return; // the user cancelled the picker
        }
        var ok = false;
        try
        {
            // Buffered under the log gate (rotation caps it at ~1 MB), so this write never
            // races the logger.
            var bytes = Log.ExportSnapshot();
            await using var output = await file.OpenStreamForWriteAsync();
            output.SetLength(0);
            await output.WriteAsync(bytes);
            ok = true;
            Log.Info($"diagnostics: log exported ({bytes.Length} bytes)");
        }
        catch (Exception ex)
        {
            // A mid-write failure (disk full, disconnected target) must not crash the app.
            Log.Warn($"diagnostics export failed: {ex.GetType().Name}");
        }
        if (!ok)
        {
            // Don't leave a half-written or empty file at the user's chosen path.
            try
            {
                await file.DeleteAsync();
            }
            catch
            {
                // best effort
            }
        }
    }
}
