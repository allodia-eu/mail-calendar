// Settings → Advanced ▸ Own AI endpoint (docs/ai.md rules 6 and 7), drawn from the core's
// ai_endpoint_form(): nothing at all while the writing assistant is not offered. The four fields
// are the form's own until Save sends them; what a save would refuse is asked of the core
// (ai_endpoint_gap) as the person types and said beside each field, and Save is held meanwhile.
// Save and Remove run in the core, the fields held with them; what a change came to is the
// record's outcome, said once (AiEndpointRules). The key field never draws the stored key back.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The order the where-it-runs choice lists the declarations in. Unknown is not offered: it is
    // what a form with nothing chosen sends.
    private static readonly JurisdictionClass[] Declarations =
        [JurisdictionClass.EuNative, JurisdictionClass.EuHosted, JurisdictionClass.NonEu];

    // The form on screen, or null while none is.
    private AiEndpointParts? _aiEndpoint;

    // What the last change came to, standing on the form until it is closed or the next change
    // starts; and the outcomes already acted on, from when Settings opened.
    private string? _aiEndpointSaid;
    private readonly AiEndpointOutcomes _aiEndpointOutcomes;

    private static AiEndpointWords AiEndpointText() => new(
        L10n.AiEndpointGapAddressMissing(),
        L10n.AiEndpointGapAddressUnusable(),
        L10n.AiEndpointGapNeedsHttps(),
        L10n.AiEndpointGapModelMissing(),
        L10n.AiEndpointGapWhereMissing(),
        L10n.AiEndpointGapRefusedEuNative(),
        L10n.AiEndpointGapRefusedEuHosted(),
        L10n.AiEndpointKeyHint(),
        L10n.AiEndpointKeyStored(),
        L10n.AiEndpointKeyDropped(),
        L10n.AiEndpointKeyNotStored());

    /// <summary>The form's group, or null while the assistant is not offered.</summary>
    private UIElement? AiEndpointGroup()
    {
        _aiEndpoint = null;
        if (_model.AiEndpointForm() is not { } form)
        {
            return null;
        }
        var parts = new AiEndpointParts(form);
        parts.Address.Header = L10n.AiEndpointAddress();
        parts.Address.PlaceholderText = L10n.AiEndpointAddressHint();
        parts.Key.Header = L10n.AiEndpointKey();
        parts.Model.Header = L10n.AiEndpointModel();
        parts.Where.Header = L10n.AiEndpointWhere();
        foreach (var label in new[] { L10n.AiEndpointWhereEuNative(), L10n.AiEndpointWhereEuHosted(), L10n.AiEndpointWhereNonEu() })
        {
            parts.Where.Items.Add(label);
        }
        NameField(parts.Address, L10n.AiEndpointAddress(), "AiEndpointAddress");
        NameField(parts.Key, L10n.AiEndpointKey(), "AiEndpointKey");
        NameField(parts.Model, L10n.AiEndpointModel(), "AiEndpointModel");
        NameField(parts.Where, L10n.AiEndpointWhere(), "AiEndpointWhere");
        AutomationProperties.SetAutomationId(parts.AddressGap, "AiEndpointAddress_Gap");
        AutomationProperties.SetAutomationId(parts.ModelGap, "AiEndpointModel_Gap");
        AutomationProperties.SetAutomationId(parts.WhereGap, "AiEndpointWhere_Gap");
        AutomationProperties.SetAutomationId(parts.KeyNote, "AiEndpointKey_Note");
        AutomationProperties.SetAutomationId(parts.Said, "AiEndpointOutcome");
        foreach (var gap in new[] { parts.AddressGap, parts.ModelGap, parts.WhereGap })
        {
            gap.Foreground = _brushes.Of(ThemePalette.Critical);
        }
        parts.Said.Closed += (_, _) => _aiEndpointSaid = null;

        parts.Root.Children.Add(parts.Address);
        parts.Root.Children.Add(parts.AddressGap);
        parts.Root.Children.Add(parts.Key);
        parts.Root.Children.Add(parts.KeyNote);
        parts.Root.Children.Add(parts.Model);
        parts.Root.Children.Add(parts.ModelGap);
        parts.Root.Children.Add(parts.Where);
        parts.Root.Children.Add(parts.WhereGap);
        parts.Root.Children.Add(parts.Said);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        buttons.Children.Add(parts.SaveSlot);
        buttons.Children.Add(parts.RemoveSlot);
        parts.Root.Children.Add(buttons);

        DrawAiEndpointFields(parts, form);
        parts.Address.TextChanged += (_, _) => DrawAiEndpointTyped(parts);
        parts.Key.PasswordChanged += (_, _) => DrawAiEndpointTyped(parts);
        parts.Model.TextChanged += (_, _) => DrawAiEndpointTyped(parts);
        parts.Where.SelectionChanged += (_, _) => DrawAiEndpointTyped(parts);
        _aiEndpoint = parts;
        RefreshAiEndpoint(parts, form);
        return Group(L10n.AiEndpointTitle(), L10n.AiEndpointIntro(), parts.Root);
    }

    // The form moved (Surface.AiEndpoint or Surface.Settings). One that appeared or went draws the
    // category again; otherwise its controls are drawn in place, and what is typed stays.
    private void OnAiEndpointChanged()
    {
        if (_rebuilding || CurrentCategory() != "advanced")
        {
            return;
        }
        var form = _model.AiEndpointForm();
        if (form is null || _aiEndpoint is null)
        {
            if ((form is null) != (_aiEndpoint is null))
            {
                ShowCategory("advanced");
            }
            return;
        }
        RefreshAiEndpoint(_aiEndpoint, form);
    }

    private void RefreshAiEndpoint(AiEndpointParts parts, AiEndpointForm form)
    {
        parts.Form = form;
        var fresh = _aiEndpointOutcomes.IsNew(form.Outcome);
        if (_aiEndpointOutcomes.Read(form.Outcome, AiEndpointText()) is { } said)
        {
            _aiEndpointSaid = said;
        }
        // A change that ended hands the fields back as it left them: the edit it stored, or the one
        // it refused. Otherwise they follow the record only while nobody has typed over them.
        if (fresh || parts.Typed() == parts.Drawn)
        {
            if (fresh && form.Outcome?.Result is AiEndpointResult.Saved or AiEndpointResult.Removed)
            {
                parts.Key.Password = string.Empty;
            }
            DrawAiEndpointFields(parts, form);
        }
        ControlComponents.HoldFields(form.Save, parts.Address, parts.Key, parts.Model, parts.Where);
        parts.Said.Message = _aiEndpointSaid ?? string.Empty;
        parts.Said.IsOpen = _aiEndpointSaid is not null;
        // Remove is drawn only while there is something to remove.
        if (form.Remove != parts.RemoveDrawn)
        {
            parts.RemoveDrawn = form.Remove;
            parts.RemoveSlot.Child = form.Remove is { } remove
                ? ControlComponents.Button(L10n.AiEndpointRemove(), remove, "AiEndpointRemove", () =>
                {
                    SayAiEndpointNothing(parts);
                    _model.ChangeAiEndpoint(new AiEndpointIntent.Remove());
                })
                : null;
        }
        DrawAiEndpointTyped(parts);
    }

    private void DrawAiEndpointFields(AiEndpointParts parts, AiEndpointForm form)
    {
        parts.Address.Text = form.Address;
        parts.Model.Text = form.Model;
        parts.Where.SelectedIndex = form.Declared is { } declared ? Array.IndexOf(Declarations, declared) : -1;
        parts.Drawn = parts.Typed();
    }

    // What a save of the fields as typed would refuse, beside each field, and Save held meanwhile.
    private void DrawAiEndpointTyped(AiEndpointParts parts)
    {
        var typed = parts.Typed();
        var gaps = _model.AiEndpointGap(typed.Address, typed.Model, typed.Declared);
        var lines = AiEndpointRules.Lines(gaps, typed, AiEndpointText());
        // What becomes of a kept key follows the address as typed.
        parts.KeyNote.Text = AiEndpointRules.KeyNote(gaps.KeptKey, AiEndpointText());
        Say(parts.AddressGap, lines.Address);
        Say(parts.ModelGap, lines.Model);
        Say(parts.WhereGap, lines.Where);
        var save = AiEndpointRules.SaveState(parts.Form.Save, gaps);
        if (save != parts.SaveDrawn)
        {
            parts.SaveDrawn = save;
            parts.SaveSlot.Child = ControlComponents.Button(
                L10n.AiEndpointSave(), save, "AiEndpointSave", () => SaveAiEndpoint(parts),
                (Style)Application.Current.Resources["AccentButtonStyle"]);
        }
    }

    private void SaveAiEndpoint(AiEndpointParts parts)
    {
        var typed = parts.Typed();
        SayAiEndpointNothing(parts);
        // The key as typed: an empty one leaves the kept key, and only Remove takes it out.
        _model.ChangeAiEndpoint(new AiEndpointIntent.Save(typed.Address, typed.Model, typed.Declared, typed.Key));
    }

    // A change starting takes down what the last one came to.
    private void SayAiEndpointNothing(AiEndpointParts parts)
    {
        _aiEndpointSaid = null;
        parts.Said.IsOpen = false;
    }

    private static void Say(TextBlock line, string? text)
    {
        line.Text = text ?? string.Empty;
        line.Visibility = text is null ? Visibility.Collapsed : Visibility.Visible;
    }

    // Each field named for assistive technology, which does not follow a header, and kept from
    // passing Enter on to the dialog, whose default button would close Settings.
    private static void NameField(Control field, string name, string automationId)
    {
        AutomationProperties.SetName(field, name);
        AutomationProperties.SetAutomationId(field, automationId);
        field.KeyDown += (_, e) =>
        {
            if (e.Key == Windows.System.VirtualKey.Enter)
            {
                e.Handled = true;
            }
        };
    }

    // The form on screen: its fields, the lines beside them, the record it last drew and the two
    // slots its record-drawn buttons sit in.
    private sealed class AiEndpointParts(AiEndpointForm form)
    {
        internal StackPanel Root { get; } = new() { Spacing = 8 };

        internal TextBox Address { get; } = new() { MinWidth = 320, HorizontalAlignment = HorizontalAlignment.Left };

        internal PasswordBox Key { get; } = new() { MinWidth = 320, HorizontalAlignment = HorizontalAlignment.Left };

        internal TextBox Model { get; } = new() { MinWidth = 320, HorizontalAlignment = HorizontalAlignment.Left };

        internal RadioButtons Where { get; } = new();

        internal TextBlock AddressGap { get; } = Line();

        internal TextBlock ModelGap { get; } = Line();

        internal TextBlock WhereGap { get; } = Line();

        internal TextBlock KeyNote { get; } = new() { TextWrapping = TextWrapping.Wrap, Opacity = 0.7 };

        internal InfoBar Said { get; } = new() { Severity = InfoBarSeverity.Error, IsClosable = true };

        internal Border SaveSlot { get; } = new();

        internal Border RemoveSlot { get; } = new();

        internal AiEndpointForm Form { get; set; } = form;

        internal AiEndpointTyped? Drawn { get; set; }

        internal ControlState? SaveDrawn { get; set; }

        internal ControlState? RemoveDrawn { get; set; }

        internal AiEndpointTyped Typed() => new(
            Address.Text,
            Key.Password,
            Model.Text,
            Where.SelectedIndex is >= 0 and var at && at < Declarations.Length ? Declarations[at] : null);

        private static TextBlock Line() => new() { TextWrapping = TextWrapping.Wrap, Visibility = Visibility.Collapsed };
    }
}
