// The composer's Draft a reply control (docs/ai.md, "Drafting a reply"): a flyout with a one-line
// intent and four chips that fill it, then a reply drafted in the person's style and put above the
// signature and the quote through the editor bundle's setComposerDraftText. Nothing is sent from
// here: the draft lands in the composer, where the person edits it.
//
// The rules (when it is offered, whose style, how the editor's answer reads) are DraftReplyGate,
// where Mailcal.Tests pins them. What is here is the WinUI and the calls into the editor.

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    // A draft is on its way; the control waits for it rather than asking twice.
    private bool _drafting;

    // The failure this control last put on the composer's one error line, so a later draft takes
    // away its own words and never another failure's.
    private string? _draftError;

    private void InitDraftReply()
    {
        var offered = _model is { } model
            && _request is { Account: not null, Key: not null } request
            && DraftReplyGate.Offered(request.Kind, model.WritingStyles.Route is not null);
        DraftReplyHost.Visibility = offered ? Visibility.Visible : Visibility.Collapsed;
        if (!offered)
        {
            return;
        }
        DraftIntentBox.PlaceholderText = L10n.ComposerDraftIntentHint();
        AutomationProperties.SetName(DraftIntentBox, L10n.ComposerDraftIntentHint());
        DraftChipYes.Content = L10n.ComposerDraftChipYes();
        DraftChipNo.Content = L10n.ComposerDraftChipNo();
        DraftChipMoreInfo.Content = L10n.ComposerDraftChipMoreInfo();
        DraftChipLater.Content = L10n.ComposerDraftChipLater();
        DraftCreateButton.Content = L10n.ComposerDraftCreate();
        UpdateDraftReplyAvailability();
    }

    // Enabled while the sender's account drafts in a style; otherwise disabled, with the reason as
    // the host's tooltip and the button's help text.
    private void UpdateDraftReplyAvailability()
    {
        if (DraftReplyHost.Visibility != Visibility.Visible || _model is null)
        {
            return;
        }
        var from = (FromBox.SelectedItem as AccountItem)?.Id;
        var hasStyle = from is not null && _model.ResolveWritingStyle(from) is not null;
        DraftReplyButton.IsEnabled = hasStyle && !_drafting;
        if (hasStyle)
        {
            DraftReplyHost.ClearValue(ToolTipService.ToolTipProperty);
            AutomationProperties.SetHelpText(DraftReplyButton, string.Empty);
        }
        else
        {
            ToolTipService.SetToolTip(DraftReplyHost, L10n.AiErrorNoStyle());
            AutomationProperties.SetHelpText(DraftReplyButton, L10n.AiErrorNoStyle());
        }
    }

    // A chip puts its words in the intent field, where the person can still change them.
    private void OnDraftChip(object sender, RoutedEventArgs e)
    {
        if (sender is Button { Content: string text })
        {
            DraftIntentBox.Text = text;
        }
    }

    private async void OnDraftCreate(object sender, RoutedEventArgs e)
    {
        DraftReplyFlyout.Hide();
        if (_drafting || !_editorReady || _model is not { } model
            || _request is not { Account: { } account, Key: { } key })
        {
            return;
        }
        try
        {
            if (!await ConfirmReplaceAsync())
            {
                return;
            }
            var before = await ReadDocumentAsync();
            SetDrafting(true);
            var from = DraftReplyGate.From((FromBox.SelectedItem as AccountItem)?.Id, account);
            var outcome = await model.DraftReplyAsync(account, key, from, DraftReplyGate.Intent(DraftIntentBox.Text));
            SetDrafting(false);
            if (outcome.Value is not { } draft)
            {
                ShowDraftError(WritingStyleText.Of(outcome.Failure ?? new AiFailure(AiProblem.Unavailable)));
                return;
            }
            // The composer stayed usable while the draft was on its way, so what the person wrote
            // meanwhile is asked about too, never replaced unseen.
            if (await ReadDocumentAsync() != before && !await ConfirmReplaceAsync())
            {
                return;
            }
            await _editor.RunAsync(
                $"window.setComposerDraftText({EditorWebViewHost.Arg(draft.Text)}, {EditorWebViewHost.Arg(draft.DraftId)})");
            ClearDraftError();
            DraftBracketsHint.Visibility = draft.Gaps.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
            DraftIntentBox.Text = string.Empty;
        }
        catch (Exception ex)
        {
            // The exception's kind only: the draft and the message are the person's mail.
            SetDrafting(false);
            Log.Warn($"composer: couldn't put the drafted reply in ({ex.GetType().Name})");
        }
    }

    // True when nothing above the signature and the quote would be lost, or the person agreed to
    // replace it.
    private async Task<bool> ConfirmReplaceAsync()
    {
        if (!DraftReplyGate.LeadHasText(await _editor.EvaluateAsync("window.composerLeadHasText()")))
        {
            return true;
        }
        var answer = await DialogHelper.ConfirmAsync(
            this.XamlRoot,
            L10n.ComposerDraftReplaceTitle(),
            L10n.ComposerDraftReplaceMessage(),
            L10n.ComposerDraftReplace());
        return answer == ContentDialogResult.Primary;
    }

    private void SetDrafting(bool drafting)
    {
        _drafting = drafting;
        DraftingRow.Visibility = drafting ? Visibility.Visible : Visibility.Collapsed;
        DraftingRing.IsActive = drafting;
        if (drafting)
        {
            // The check-the-brackets line belongs to the previous draft.
            DraftBracketsHint.Visibility = Visibility.Collapsed;
            ClearDraftError();
        }
        UpdateDraftReplyAvailability();
    }

    private void ShowDraftError(string message)
    {
        _draftError = message;
        ShowError(message);
    }

    private void ClearDraftError()
    {
        if (_draftError is not null && PrepareError.Text == _draftError)
        {
            PrepareError.Visibility = Visibility.Collapsed;
        }
        _draftError = null;
    }
}
