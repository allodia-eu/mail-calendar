// The Signatures settings category (docs/signatures.md): the library, write once, reuse on any
// account, above the per-account defaults, one "For new messages" and one "For replies or forwards"
// picker each. State lives in the Rust core (the SignaturesSnapshot); these render it and dispatch
// the setters, which re-signal Surface.Settings.
//
// Two things the layout is deliberate about, and they match macOS/iOS/Android exactly. The library
// comes first because an account picker with nothing to pick is meaningless, a first-time user has
// to write a signature before the defaults mean anything. And "None" is a real option in both
// pickers rather than a separate enable switch: "None in both" already says "this account sends no
// signature", and a second control that could disagree with the pickers is a bug waiting to happen.
//
// Split into its own partial to keep SettingsDialog.cs under the 500-line limit; the body editor is
// a third (SettingsDialog.SignatureEditor.cs), because it hosts a WebView2 and its own gates.

using System.Linq;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // Which signature the inline editor is open for, or null when the category shows its list. A
    // non-null Id is an edit, a null Id inside a non-null value is a create, the editor is the same
    // either way, only its title and what Save dispatches differ.
    private EditingSignature? _editingSignature;

    // The signature the user has asked to delete, awaiting the inline confirmation. A nested
    // ContentDialog is not allowed inside this one, so the row reveals the warning in place, the
    // same shape the destructive database reset uses in BuildAdvanced.
    private string? _deletingSignature;

    // --- Signatures: the library, then each account's two slots ---------------

    private UIElement BuildSignatures()
    {
        if (_editingSignature is { } editing)
        {
            return BuildSignatureEditor(editing);
        }

        var snapshot = _model.Signatures;
        // Counts only, never a name or a body (docs/logging.md). Worth keeping: an empty panel
        // over a populated core looks exactly like a core with nothing in it, and this is the one
        // line that tells the two apart.
        Log.Info($"settings: signatures panel, {snapshot.Signatures.Length} in the library, {snapshot.Accounts.Length} account(s), connected={_model.HasCore}");
        var panel = new StackPanel { Spacing = 20 };
        panel.Children.Add(Group(
            L10n.SettingsSignaturesLibraryHeading(),
            L10n.SettingsSignaturesLibraryDescription(),
            SignatureLibrary(snapshot.Signatures)));
        panel.Children.Add(Group(
            L10n.SettingsSignaturesDefaultsHeading(),
            L10n.SettingsSignaturesDefaultsDescription(),
            AccountSignatureDefaults(snapshot)));
        return panel;
    }

    // Every signature the user has written, each editable and deletable, plus the button that writes
    // a new one.
    private UIElement SignatureLibrary(IReadOnlyList<SignatureRow> signatures)
    {
        var panel = new StackPanel { Spacing = 4 };
        if (signatures.Count == 0)
        {
            panel.Children.Add(new TextBlock
            {
                Text = L10n.SettingsSignaturesEmpty(),
                TextWrapping = TextWrapping.Wrap,
                Opacity = 0.7,
            });
        }
        foreach (var signature in signatures)
        {
            panel.Children.Add(SignatureRowControl(signature));
        }

        var add = new Button { Content = L10n.SettingsSignaturesAdd(), Margin = new Thickness(0, 8, 0, 0) };
        add.Click += (_, _) => OpenSignatureEditor(
            new EditingSignature(null, L10n.SettingsSignaturesDefaultName(), string.Empty));
        panel.Children.Add(add);
        return panel;
    }

    // One library row: the name, Edit, Delete, and, once Delete is pressed, the confirmation in
    // place of a second dialog. The name is not itself clickable: Delete sits beside it, and a stray
    // click that opens an editor is recoverable while one that deletes is not.
    private UIElement SignatureRowControl(SignatureRow signature)
    {
        var row = new Grid { ColumnSpacing = 8, MinHeight = 32 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var name = new TextBlock
        {
            Text = signature.Name,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };
        Grid.SetColumn(name, 0);
        var edit = new Button { Content = L10n.SettingsSignaturesEdit() };
        edit.Click += (_, _) => OpenSignatureEditor(new EditingSignature(
            signature.Id,
            signature.Name,
            _model.SignatureHtml(signature.Id) ?? string.Empty));
        Grid.SetColumn(edit, 1);
        var delete = new Button { Content = L10n.SettingsSignaturesDelete() };
        delete.Click += (_, _) => Apply(() => _deletingSignature = signature.Id);
        Grid.SetColumn(delete, 2);
        row.Children.Add(name);
        row.Children.Add(edit);
        row.Children.Add(delete);

        if (_deletingSignature != signature.Id)
        {
            return row;
        }

        // The confirmation, inline. It says what deleting costs beyond this list, every account
        // pointing at this signature loses it, which the core does in one place so no client can
        // forget a teardown path.
        var stack = new StackPanel { Spacing = 6 };
        stack.Children.Add(row);
        stack.Children.Add(new TextBlock
        {
            Text = L10n.SettingsSignaturesDeleteTitle(),
            TextWrapping = TextWrapping.Wrap,
        });
        stack.Children.Add(new TextBlock
        {
            Text = L10n.SettingsSignaturesDeleteMessage(),
            TextWrapping = TextWrapping.Wrap,
            Opacity = 0.7,
        });
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var confirm = new Button { Content = L10n.SettingsSignaturesDelete() };
        confirm.Click += (_, _) => Apply(() =>
        {
            _deletingSignature = null;
            _model.DeleteSignature(signature.Id);
        });
        var cancel = new Button { Content = L10n.ActionCancel() };
        cancel.Click += (_, _) => Apply(() => _deletingSignature = null);
        buttons.Children.Add(confirm);
        buttons.Children.Add(cancel);
        stack.Children.Add(buttons);
        return new Border
        {
            Background = (Brush)Application.Current.Resources["LayerFillColorDefaultBrush"],
            CornerRadius = new CornerRadius(6),
            Padding = new Thickness(10),
            Child = stack,
        };
    }

    // For each configured account, which signature a new message opens with and which a reply or
    // forward does, independently, each with "None".
    private UIElement AccountSignatureDefaults(SignaturesSnapshot snapshot)
    {
        if (snapshot.Accounts.Length == 0)
        {
            return new TextBlock
            {
                Text = L10n.SettingsAccountsEmpty(),
                TextWrapping = TextWrapping.Wrap,
                Opacity = 0.7,
            };
        }
        var panel = new StackPanel { Spacing = 16 };
        foreach (var account in snapshot.Accounts)
        {
            var card = new StackPanel { Spacing = 4 };
            // With one account the address is still shown: the setting is per account, and a user
            // who later adds a second must not have to relearn that.
            card.Children.Add(Heading(account.Email));
            card.Children.Add(SignatureSlotPicker(
                L10n.SettingsSignaturesNewMessageLabel(),
                snapshot.Signatures,
                account.NewMessage,
                id => _model.SetAccountSignature(account.AccountId, SignatureSlotKind.NewMessage, id)));
            card.Children.Add(SignatureSlotPicker(
                L10n.SettingsSignaturesReplyForwardLabel(),
                snapshot.Signatures,
                account.ReplyForward,
                id => _model.SetAccountSignature(account.AccountId, SignatureSlotKind.ReplyForward, id)));
            panel.Children.Add(card);
        }
        return panel;
    }

    // One slot's picker: the library plus "None", the label above the control rather than beside it
    // (two rows holding the same signature would otherwise be indistinguishable).
    private UIElement SignatureSlotPicker(
        string label,
        IReadOnlyList<SignatureRow> signatures,
        string? selected,
        Action<string?> onSelect)
    {
        var options = new List<SignatureOption> { new(null, L10n.SettingsSignaturesNone()) };
        options.AddRange(signatures.Select(s => new SignatureOption(s.Id, s.Name)));
        var box = new ComboBox
        {
            Header = label,
            MinWidth = 240,
            ItemsSource = options,
            // A slot can name a signature that has since been deleted only if the core failed to
            // clear it (it clears every assignment on delete), so falling back to "None" here is a
            // display detail, not a second teardown path.
            SelectedItem = options.FirstOrDefault(o => o.Id == selected) ?? options[0],
        };
        box.SelectionChanged += (_, _) =>
        {
            if (!_rebuilding && box.SelectedItem is SignatureOption option)
            {
                onSelect(option.Id);
            }
        };
        return box;
    }

    // Opens the inline body editor for a create or an edit, and re-renders the category into it.
    private void OpenSignatureEditor(EditingSignature editing) => Apply(() =>
    {
        _deletingSignature = null;
        _editingSignature = editing;
    });

    /// <summary>What the inline signature editor is open for.</summary>
    /// <param name="Id">The signature being edited, or <c>null</c> for a create.</param>
    /// <param name="Name">The name the editor opens with.</param>
    /// <param name="BodyHtml">The stored body an edit opens with; empty for a create.</param>
    private sealed record EditingSignature(string? Id, string Name, string BodyHtml);

    // One entry in a slot picker; ToString is the label so the ComboBox shows it directly. A null Id
    // is "None", an assignment the user can actually make, not the absence of one.
    private sealed record SignatureOption(string? Id, string Label)
    {
        public override string ToString() => Label;
    }
}
