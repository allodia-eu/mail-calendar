// The Signatures settings category (docs/signatures.md): the library above each account's two
// slots, "For new messages" and "For replies or forwards". It draws the core's SignaturesPage and
// keeps no rule of its own: every control comes through ControlComponents with the page's state,
// and every change is a SignatureIntent, which returns at once (docs/waiting.md).
//
// The library comes first because an account picker with nothing to pick is meaningless. "None"
// is a real option in both pickers rather than a separate switch: "None in both" already says
// "this account sends no signature".
//
// The body editor is its own partial (SettingsDialog.SignatureEditor.cs), because it hosts a
// WebView2 and its own gates.

using System.Linq;
using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // What the inline editor is open on, or null when the category shows its list.
    private EditingSignature? _editingSignature;

    // The signature whose deletion is being asked about inline: a nested ContentDialog is not
    // allowed inside this one, so the row asks in place, as the database reset does.
    private string? _deletingSignature;

    private UIElement BuildSignatures()
    {
        var page = _model.SignaturesPage();
        if (_editingSignature is { } editing)
        {
            return BuildSignatureEditor(editing, page);
        }

        // Counts only, never a name or a body (docs/logging.md). An empty panel over a populated
        // core looks exactly like a core with nothing in it, and this line tells the two apart.
        Log.Info($"settings: signatures panel, {page.Signatures.Length} in the library, {page.Accounts.Length} account(s), connected={_model.HasCore}");
        var panel = new StackPanel { Spacing = 20 };
        panel.Children.Add(Group(
            L10n.SettingsSignaturesLibraryHeading(),
            L10n.SettingsSignaturesLibraryDescription(),
            SignatureLibrary(page)));
        panel.Children.Add(Group(
            L10n.SettingsSignaturesDefaultsHeading(),
            L10n.SettingsSignaturesDefaultsDescription(),
            AccountSignatureDefaults(page)));
        return panel;
    }

    // The page moved (Surface.Signatures or Surface.Settings). An open editor keeps what is typed
    // in it and reads only its controls and the outcome; the list is drawn again.
    private void OnSignaturesChanged()
    {
        if (_rebuilding || CurrentCategory() != "signatures")
        {
            return;
        }
        if (_editingSignature is { } editing)
        {
            RefreshSignatureEditor(editing, _model.SignaturesPage());
            return;
        }
        ShowCategory("signatures");
    }

    // Every signature the user has written, each editable and deletable, then New signature.
    private UIElement SignatureLibrary(SignaturesPage page)
    {
        var panel = new StackPanel { Spacing = 4 };
        if (page.Signatures.Length == 0)
        {
            panel.Children.Add(new TextBlock
            {
                Text = L10n.SettingsSignaturesEmpty(),
                TextWrapping = TextWrapping.Wrap,
                Opacity = 0.7,
            });
        }
        for (var at = 0; at < page.Signatures.Length; at++)
        {
            panel.Children.Add(SignatureRowControl(page.Signatures[at], at));
        }

        var add = ControlComponents.Button(L10n.SettingsSignaturesAdd(), page.Add, "SignatureAdd", () =>
        {
            var now = _model.SignaturesPage();
            OpenSignatureEditor(new EditingSignature(
                SignatureEditorSession.New(now), L10n.SettingsSignaturesDefaultName(), string.Empty, IsNew: true));
        });
        add.Margin = new Thickness(0, 8, 0, 0);
        panel.Children.Add(add);
        return panel;
    }

    // One library row: the name, Edit and Delete, and beneath it the question Delete asks. The
    // name is not itself clickable: Delete sits beside it, and a stray click that opens an editor is
    // recoverable while one that deletes is not.
    private UIElement SignatureRowControl(SignatureEntry signature, int at)
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
        var edit = ControlComponents.Button(L10n.SettingsSignaturesEdit(), signature.Edit, $"SignatureEdit_{at}", () =>
        {
            var now = _model.SignaturesPage();
            OpenSignatureEditor(new EditingSignature(
                SignatureEditorSession.Edit(signature, now),
                signature.Name,
                _model.SignatureHtml(signature.Id) ?? string.Empty,
                IsNew: false));
        });
        Grid.SetColumn(edit, 1);

        // Deleting it clears it from every account that uses it, which the question says; the core
        // does that teardown, so no client can forget it.
        var confirm = new ConfirmPanel(
            L10n.SettingsSignaturesDeleteTitle(),
            L10n.SettingsSignaturesDeleteMessage(),
            L10n.SettingsSignaturesDelete(),
            $"SignatureDeleteAsk_{at}",
            yes =>
            {
                _deletingSignature = null;
                if (yes)
                {
                    _model.ChangeSignatures(new SignatureIntent.Delete(signature.Id));
                }
            });
        // Held while another change runs, as Delete is.
        confirm.Enabled = signature.Delete.Enabled;
        var delete = ControlComponents.Button(L10n.SettingsSignaturesDelete(), signature.Delete, $"SignatureDelete_{at}", () =>
        {
            _deletingSignature = signature.Id;
            confirm.Show();
        });
        Grid.SetColumn(delete, 2);
        row.Children.Add(name);
        row.Children.Add(edit);
        row.Children.Add(delete);

        // Still asked when the page is drawn again.
        if (_deletingSignature == signature.Id)
        {
            confirm.Show();
        }
        var stack = new StackPanel { Spacing = 6 };
        stack.Children.Add(row);
        stack.Children.Add(confirm.Element);
        return stack;
    }

    // For each account used for mail, which signature a new message opens with and which a reply or
    // forward does, independently, each with "None".
    private UIElement AccountSignatureDefaults(SignaturesPage page)
    {
        if (page.Accounts.Length == 0)
        {
            return new TextBlock
            {
                Text = L10n.SettingsAccountsEmpty(),
                TextWrapping = TextWrapping.Wrap,
                Opacity = 0.7,
            };
        }
        var panel = new StackPanel { Spacing = 16 };
        for (var at = 0; at < page.Accounts.Length; at++)
        {
            var account = page.Accounts[at];
            var card = new StackPanel { Spacing = 4 };
            // With one account the address is still shown: the setting is per account, and a user
            // who later adds a second must not have to relearn that.
            card.Children.Add(Heading(account.Email));
            card.Children.Add(SignatureSlotPicker(
                L10n.SettingsSignaturesNewMessageLabel(),
                page.Signatures,
                account.NewMessage,
                $"SignatureSlotNew_{at}",
                id => new SignatureIntent.Assign(account.AccountId, SignatureSlotKind.NewMessage, id)));
            card.Children.Add(SignatureSlotPicker(
                L10n.SettingsSignaturesReplyForwardLabel(),
                page.Signatures,
                account.ReplyForward,
                $"SignatureSlotReply_{at}",
                id => new SignatureIntent.Assign(account.AccountId, SignatureSlotKind.ReplyForward, id)));
            panel.Children.Add(card);
        }
        return panel;
    }

    // One slot's picker: "None", then the library. The label sits above the control, because two
    // rows holding the same signature would otherwise be indistinguishable.
    private StackPanel SignatureSlotPicker(
        string label,
        SignatureEntry[] signatures,
        SignaturePicker picker,
        string automationId,
        Func<string?, SignatureIntent> assign)
    {
        var ids = new List<string?> { null };
        ids.AddRange(signatures.Select(s => s.Id));
        var labels = new List<string> { L10n.SettingsSignaturesNone() };
        labels.AddRange(signatures.Select(s => s.Name));
        return ControlComponents.ComboBox(
            label,
            labels,
            Math.Max(0, ids.IndexOf(picker.Selected)),
            picker.Control,
            automationId,
            pick => _model.ChangeSignatures(assign(ids[pick])));
    }

    // Opens the inline body editor and draws the category into it.
    private void OpenSignatureEditor(EditingSignature editing) => Apply(() =>
    {
        _deletingSignature = null;
        _editingSignature = editing;
    });

    /// <summary>What the inline signature editor is open on.</summary>
    /// <param name="Session">The signature it saves under and the outcomes it has seen.</param>
    /// <param name="Name">The name it opens with.</param>
    /// <param name="BodyHtml">The stored body an edit opens with; empty for a new signature.</param>
    /// <param name="IsNew">Whether it writes a new signature, which titles it.</param>
    private sealed record EditingSignature(SignatureEditorSession Session, string Name, string BodyHtml, bool IsNew);
}
