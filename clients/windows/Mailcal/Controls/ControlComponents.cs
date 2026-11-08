// The controls a page draws from the core's ControlState (docs/waiting.md): a switch, a button, a
// text box, a combo box and a confirmation. Each is held while it cannot be used and shows a ring
// beside it while work it started runs, so every page waits the same way.
//
// The confirmation is inline, beneath what it asks about: Settings is a ContentDialog, and a nested
// one is not allowed.

using System;
using System.Collections.Generic;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Controls;

internal static class ControlComponents
{
    /// <summary>The ring beside a control whose work is running; collapsed otherwise.</summary>
    internal static ProgressRing Running(ControlState control, string automationId)
    {
        var ring = new ProgressRing
        {
            IsActive = control.Running,
            Width = 16,
            Height = 16,
            VerticalAlignment = VerticalAlignment.Bottom,
            Margin = new Thickness(0, 0, 0, 8),
            Visibility = control.Running ? Visibility.Visible : Visibility.Collapsed,
        };
        AutomationProperties.SetAutomationId(ring, automationId + "_Running");
        return ring;
    }

    /// <summary>A control with its ring beside it.</summary>
    private static StackPanel Beside(UIElement control, ControlState state, string automationId)
    {
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        line.Children.Add(control);
        line.Children.Add(Running(state, automationId));
        return line;
    }

    /// <summary>A button. <paramref name="click"/> runs only while it is enabled.</summary>
    internal static StackPanel Button(string label, ControlState control, string automationId, Action click)
    {
        var button = new Button { Content = label, IsEnabled = control.Enabled };
        AutomationProperties.SetAutomationId(button, automationId);
        button.Click += (_, _) => click();
        return Beside(button, control, automationId);
    }

    /// <summary>
    /// A text box named <paramref name="name"/>, which the page's own heading stands above, and
    /// which tells <paramref name="commit"/> what was typed on Enter,
    /// when it loses focus and when the page is redrawn without it, and only when it differs from
    /// what it last told, starting from <paramref name="text"/>: never on every keystroke, which
    /// would store a half-typed value and redraw the page under the cursor, and never twice for
    /// one edit, as Enter and then the focus leaving would. A redraw for another change takes the
    /// box away; the core runs what was typed once that change ends.
    /// </summary>
    internal static StackPanel TextBox(
        string name, string text, ControlState control, string automationId, Action<string> commit)
    {
        var box = new TextBox
        {
            PlaceholderText = name,
            Text = text,
            MinWidth = 260,
            HorizontalAlignment = HorizontalAlignment.Left,
            IsEnabled = control.Enabled,
        };
        AutomationProperties.SetName(box, name);
        AutomationProperties.SetAutomationId(box, automationId);
        var sent = text;
        void Send()
        {
            if (box.Text != sent)
            {
                sent = box.Text;
                commit(sent);
            }
        }
        box.LostFocus += (_, _) => Send();
        box.Unloaded += (_, _) => Send();
        box.KeyDown += (_, e) =>
        {
            if (e.Key == Windows.System.VirtualKey.Enter)
            {
                Send();
            }
        };
        return Beside(box, control, automationId);
    }

    /// <summary>
    /// A combo box under its header: <paramref name="labels"/> listed, <paramref name="selected"/>
    /// shown. <paramref name="pick"/> is told the position the person picked, and only when it
    /// differs from <paramref name="selected"/>.
    /// </summary>
    internal static StackPanel ComboBox(
        string header, IReadOnlyList<string> labels, int selected, ControlState control, string automationId,
        Action<int> pick)
    {
        var box = new ComboBox { Header = header, MinWidth = 260, IsEnabled = control.Enabled };
        AutomationProperties.SetName(box, header);
        AutomationProperties.SetAutomationId(box, automationId);
        foreach (var label in labels)
        {
            box.Items.Add(label);
        }
        box.SelectedIndex = selected;
        box.SelectionChanged += (_, _) =>
        {
            if (box.SelectedIndex >= 0 && box.SelectedIndex != selected)
            {
                pick(box.SelectedIndex);
            }
        };
        return Beside(box, control, automationId);
    }
}

/// <summary>A switch with its note beneath. The person's flip is told to the page, which answers
/// by drawing the core's next record, or by asking first.</summary>
internal sealed class ControlToggle
{
    private readonly ToggleSwitch _toggle;
    private readonly bool _on;
    private readonly ControlState _control;
    private bool _restoring;

    internal ControlToggle(
        string header, string? note, bool on, ControlState control, string automationId, Action<bool> change)
    {
        _on = on;
        _control = control;
        _toggle = new ToggleSwitch { Header = header, IsOn = on, IsEnabled = control.Enabled };
        AutomationProperties.SetAutomationId(_toggle, automationId);
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        line.Children.Add(_toggle);
        line.Children.Add(ControlComponents.Running(control, automationId));
        Element = new StackPanel { Spacing = 2 };
        Element.Children.Add(line);
        if (note is not null)
        {
            Element.Children.Add(new TextBlock { Text = note, TextWrapping = TextWrapping.Wrap, Opacity = 0.7 });
        }
        _toggle.Toggled += (_, _) =>
        {
            if (!_restoring)
            {
                change(_toggle.IsOn);
            }
        };
    }

    internal StackPanel Element { get; }

    /// <summary>Holds the switch where the person left it while a question about it is open.</summary>
    internal void Hold() => _toggle.IsEnabled = false;

    /// <summary>Draws the switch off and held: a question about switching it off is still open
    /// from before the page was drawn again.</summary>
    internal void HoldOff()
    {
        _restoring = true;
        _toggle.IsOn = false;
        _restoring = false;
        Hold();
    }

    /// <summary>Puts the switch back where the core drew it: the question was answered no.</summary>
    internal void Restore()
    {
        _restoring = true;
        _toggle.IsOn = _on;
        _restoring = false;
        _toggle.IsEnabled = _control.Enabled;
    }
}

/// <summary>Asks before a change that deletes something: <paramref name="action"/> does it, Cancel
/// does not. Collapsed until <see cref="Show"/>; either answer collapses it again.</summary>
internal sealed class ConfirmPanel
{
    private readonly Button _confirm;

    internal ConfirmPanel(string title, string message, string action, string automationId, Action<bool> answer)
    {
        Element = new StackPanel { Spacing = 8, Visibility = Visibility.Collapsed };
        Element.Children.Add(new TextBlock { Text = title, FontWeight = FontWeights.SemiBold });
        var text = new TextBlock { Text = message, TextWrapping = TextWrapping.Wrap };
        AutomationProperties.SetAutomationId(text, automationId + "Message");
        Element.Children.Add(text);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var cancel = new Button { Content = L10n.ActionCancel() };
        _confirm = new Button { Content = action };
        AutomationProperties.SetAutomationId(_confirm, automationId + "Confirm");
        buttons.Children.Add(cancel);
        buttons.Children.Add(_confirm);
        Element.Children.Add(buttons);
        cancel.Click += (_, _) =>
        {
            Element.Visibility = Visibility.Collapsed;
            answer(false);
        };
        _confirm.Click += (_, _) =>
        {
            Element.Visibility = Visibility.Collapsed;
            answer(true);
        };
    }

    internal StackPanel Element { get; }

    /// <summary>Whether the action can be pressed: held while another change runs, as the control
    /// it asks about is. Cancel always can.</summary>
    internal bool Enabled
    {
        set => _confirm.IsEnabled = value;
    }

    internal void Show() => Element.Visibility = Visibility.Visible;
}
