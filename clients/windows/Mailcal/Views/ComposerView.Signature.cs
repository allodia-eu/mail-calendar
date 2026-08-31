// The composer's signature half (docs/signatures.md): what it opens with, what happens when the
// sender changes, and the per-message override. Split out of ComposerView.xaml.cs so that file stays
// the composer and this one is the one concern, the same split Apple makes between
// RichComposerView and RichComposerEditor, and Android between RichComposeScreen and
// ComposerSignatures.
//
// The pure rules (which slot a kind seeds from, the resolution precedence, the seed payload) are in
// Services/ComposerSignatures.cs, where the test project can reach them. What is left here is the
// WinUI: the menu, and calling the editor seam.

using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    // The library, read once when the composer opens: it feeds the menu, which is built once. The
    // BODIES are never cached, those are resolved on every seed and every swap, because an
    // assignment can change under an open composer.
    private IReadOnlyList<SignatureRow> _signatureLibrary = Array.Empty<SignatureRow>();

    // The user's explicit choice for THIS message, or null while it follows the account. Null is the
    // state the composer opens in, and the one a From change re-resolves; once the user picks, the
    // choice sticks across a From change, because they chose it for this message and silently
    // replacing it would undo a deliberate act.
    private SignatureChoice? _signatureChoice;

    // Builds the Signature control. Hidden entirely while the library is empty: a picker whose only
    // entry is "None" tells the user nothing, and every platform hides it the same way.
    private void InitSignatures()
    {
        _signatureChoice = null;
        // An assistant's draft carries a body someone else wrote, with its own sign-off, so it gets
        // no signature and no picker, the same call macOS makes by handing that composer no
        // signature library at all (docs/mcp.md).
        _signatureLibrary = _request?.SeedsSignature == true
            ? _model?.Signatures.Signatures ?? (IReadOnlyList<SignatureRow>)Array.Empty<SignatureRow>()
            : Array.Empty<SignatureRow>();
        if (_signatureLibrary.Count == 0)
        {
            SignatureButton.Visibility = Visibility.Collapsed;
            return;
        }
        SignatureButton.Visibility = Visibility.Visible;
        RebuildSignatureMenu();
    }

    // The menu: the library plus None, the current choice carrying a checkmark. It is rebuilt rather
    // than toggled because the checkmark moves, and because "which one is current" is derived from
    // the resolution, not stored on the items.
    private void RebuildSignatureMenu()
    {
        var current = CurrentSignature()?.Id;
        SignatureMenu.Items.Clear();
        SignatureMenu.Items.Add(SignatureMenuItem(L10n.SettingsSignaturesNone(), current is null, null));
        foreach (var signature in _signatureLibrary)
        {
            SignatureMenu.Items.Add(SignatureMenuItem(signature.Name, current == signature.Id, signature.Id));
        }
    }

    // A RadioMenuFlyoutItem, not a ToggleMenuFlyoutItem: this is a choice of exactly ONE from a set,
    // which is what the radio variant means. It draws the checkmark itself, groups the entries so
    // only one can carry it, and announces "selected" to a screen reader, where a check glyph on a
    // plain item would be decoration nobody using one could perceive. Which entry is current stays
    // DERIVED (from the account's assignment, or the user's explicit choice) and is re-derived on
    // every rebuild; assigning IsChecked does not raise Click, so re-marking cannot re-enter this.
    private MenuFlyoutItemBase SignatureMenuItem(string label, bool selected, string? id)
    {
        var item = new RadioMenuFlyoutItem
        {
            Text = label,
            GroupName = "composer-signature",
            IsChecked = selected,
        };
        item.Click += async (_, _) =>
        {
            // Picking from the menu is an explicit choice, including "None", which is why it is a
            // SignatureChoice carrying a null id rather than a null choice. A null choice would mean
            // "follow the account", and the next From change would put a signature back on a message
            // the user had just taken it off.
            _signatureChoice = new SignatureChoice(id);
            _headersDirty = true;
            await ApplySignatureAsync();
        };
        return item;
    }

    // The signature this message should carry right now, per the shared precedence rule.
    private SignatureBody? CurrentSignature()
    {
        if (_model is not { } model || _request is not { } request || !request.SeedsSignature)
        {
            return null;
        }
        return ComposerSignatures.Resolve(
            _signatureChoice,
            (FromBox.SelectedItem as AccountItem)?.Id,
            request.Kind,
            model.ResolveSignature,
            model.SignatureBodyOf);
    }

    // Pushes the resolved signature into the editor and re-marks the menu. Safe to call at any time:
    // the seam replaces only THIS message's signature region, the one that is a direct child of the
    // editor, never a quoted original's, so the user's typed text, their trimming of the quote, and
    // the caret all stay where they are.
    private async Task ApplySignatureAsync()
    {
        var seed = ComposerSignatures.SeedJson(CurrentSignature());
        var argument = seed is null ? "null" : EditorWebViewHost.Arg(seed);
        await _editor.RunAsync($"window.setComposerSignature({argument})");
        if (SignatureButton.Visibility == Visibility.Visible)
        {
            RebuildSignatureMenu();
        }
    }

    // The From account changed. Re-resolve, so a work signature never goes out under a personal
    // address, the failure the setting exists to prevent, which is why it is automatic rather than
    // a reminder. An explicit per-message choice resolves to itself and so survives untouched.
    //
    // No-ops until the editor's hooks exist: Init populates and selects the box before then, and
    // that programmatic selection is not the user changing sender.
    private async void OnFromChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!_editorReady)
        {
            return;
        }
        try
        {
            await ApplySignatureAsync();
        }
        catch (Exception ex)
        {
            Log.Warn($"composer: couldn't swap the signature for the new sender ({ex.GetType().Name})");
        }
    }
}
