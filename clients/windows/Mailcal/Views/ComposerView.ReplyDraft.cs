// Draft a reply in the composer (docs/ai.md rules 17 to 24), drawn from the core's
// composer_reply_draft(composition, from), read again on Surface.ReplyDraft, on Surface.WritingStyle
// and when From changes; null draws nothing. The control sits in the action row beside Signature
// and opens the instruction's Flyout; Drafting, a failure and the card are among the composer's
// notes. What the control does, whether it is held and why, whether to ask before replacing or at
// Send, and every tick are the core's: this words them and sends each press as its intent.
//
// The draft reaches the editor as the core's script, run once per serial in each editor, a composer
// moved into its own window following the draft already in its body; the editor's every message is
// handed to the core, whose report says what is left of the draft.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    // The record last drawn, the last draft run in this editor, and the questions on screen.
    private ComposerReplyDraft? _replyDraft;
    private ulong _draftScriptRun;
    private bool _askingArrived;
    private Flyout? _draftFlyout;
    private ControlState? _drawnControl;
    // What the open flyout shows: its line, whether it shows a hold instead, and Draft's state.
    private TextBox? _draftLine;
    private bool _flyoutHeld;
    private ControlState? _flyoutDraft;

    private void InitReplyDraft(MailboxModel model)
    {
        model.ReplyDraftChanged += RefreshReplyDraft;
        RefreshReplyDraft();
    }

    private void TeardownReplyDraft()
    {
        if (_model is not null)
        {
            _model.ReplyDraftChanged -= RefreshReplyDraft;
        }
        _draftFlyout?.Hide();
    }

    private string? SendingFrom() => (FromBox.SelectedItem as AccountItem)?.Id;

    /// <summary>Reads the record again and draws it: the control, the notes, the editor's draft and
    /// a draft that waits to be asked about.</summary>
    private void RefreshReplyDraft()
    {
        if (_model is null || string.IsNullOrEmpty(_composition))
        {
            return;
        }
        var record = _model.ComposerReplyDraft(_composition, SendingFrom());
        _replyDraft = record;
        if (record is null)
        {
            ReplyDraftSlot.Child = null;
            ReplyDraftNotes.Children.Clear();
            ReplyDraftNotes.Visibility = Visibility.Collapsed;
            _draftFlyout?.Hide();
            return;
        }
        // Built again only when its state moves, so a read that changes nothing keeps its focus.
        if (ReplyDraftSlot.Child is null || _drawnControl != record.Control)
        {
            ReplyDraftSlot.Child = ControlComponents.Button(L10n.ComposerDraftReply(), record.Control, "ComposerDraftReply", OpenDraftFlyout);
            _drawnControl = record.Control;
        }
        DrawReplyDraftNotes(record);
        // An open flyout is built again only when what it shows moves, keeping what was typed.
        if (_draftFlyout is { IsOpen: true } open && (Held(record) != _flyoutHeld || record.Draft != _flyoutDraft))
        {
            var typed = _draftLine?.Text;
            open.Content = DraftFlyoutContent(record);
            if (typed is not null && _draftLine is { } line)
            {
                line.Text = typed;
                line.SelectionStart = typed.Length;
                line.Focus(FocusState.Programmatic);
            }
        }
        _ = RunDraftScriptAsync(record);
        _ = AskArrivedAsync(record);
    }

    // The instruction's input, on the control: its line and suggestions with Draft, or why Draft
    // cannot be pressed with the remedy in place of the field.
    private void OpenDraftFlyout()
    {
        if (_replyDraft is not { } record || ReplyDraftSlot.Child is not StackPanel { Children: [Button anchor, ..] })
        {
            return;
        }
        _draftFlyout = new Flyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedLeft, Content = DraftFlyoutContent(record) };
        _draftFlyout.ShowAt(anchor);
    }

    // A hold the flyout draws in place of the line; a message gone is a note instead.
    private static bool Held(ComposerReplyDraft record) => record.HeldBy is { Failure: not AiFailure.MessageGone };

    private UIElement DraftFlyoutContent(ComposerReplyDraft record)
    {
        var panel = new StackPanel { Spacing = 10, Width = 360 };
        AutomationProperties.SetAutomationId(panel, "ComposerDraftFlyout");
        _flyoutHeld = Held(record);
        _flyoutDraft = record.Draft;
        _draftLine = null;
        if (_flyoutHeld && record.HeldBy is { } held)
        {
            var reason = new TextBlock { Text = AiWords.Catalog().Of(held.Failure), TextWrapping = TextWrapping.Wrap };
            AutomationProperties.SetAutomationId(reason, "ComposerDraftHeld");
            panel.Children.Add(reason);
            if (DraftRemedy(held.Remedy, "ComposerDraftHeldRemedy") is { } remedy)
            {
                panel.Children.Add(remedy);
            }
            return panel;
        }
        var line = new TextBox
        {
            Text = record.Instruction,
            PlaceholderText = L10n.ComposerDraftIntentHint(),
            TextWrapping = TextWrapping.Wrap,
        };
        AutomationProperties.SetName(line, L10n.ComposerDraftIntentHint());
        AutomationProperties.SetAutomationId(line, "ComposerDraftInstruction");
        line.KeyDown += (_, e) =>
        {
            if (e.Key == Windows.System.VirtualKey.Enter)
            {
                e.Handled = true;
                _ = DraftAsync(line.Text);
            }
        };
        panel.Children.Add(line);
        _draftLine = line;
        var suggestions = new FlowPanel { Spacing = 6 };
        var said = Words;
        foreach (var suggestion in record.Suggestions)
        {
            var words = said.Of(suggestion);
            var chip = new Button { Content = words, Padding = new Thickness(10, 3, 10, 4), CornerRadius = new CornerRadius(12) };
            AutomationProperties.SetAutomationId(chip, $"ComposerDraftSuggestion_{suggestion}");
            chip.Click += (_, _) =>
            {
                line.Text = words;
                line.Focus(FocusState.Programmatic);
                line.SelectionStart = line.Text.Length;
            };
            suggestions.Children.Add(chip);
        }
        panel.Children.Add(suggestions);
        var draft = ControlComponents.Button(L10n.ComposerDraftCreate(), record.Draft, "ComposerDraftCreate", () => _ = DraftAsync(line.Text),
            (Style)Application.Current.Resources["AccentButtonStyle"]);
        draft.HorizontalAlignment = HorizontalAlignment.Right;
        panel.Children.Add(draft);
        return panel;
    }

    private static ReplyDraftWords Words => new(
        L10n.ComposerDraftChipYes(), L10n.ComposerDraftChipNo(), L10n.ComposerDraftChipMoreInfo(), L10n.ComposerDraftChipLater(),
        L10n.ComposerTaskFillIn);

    // Draft, asking first while what is above the signature and the quote is the person's own.
    private async Task DraftAsync(string instruction)
    {
        if (_model is null || _replyDraft is not { Draft.Enabled: true } record)
        {
            return;
        }
        var replace = false;
        if (record.AskBeforeReplacing)
        {
            _draftFlyout?.Hide();
            replace = await AskReplaceAsync();
            if (!replace)
            {
                return;
            }
        }
        _draftFlyout?.Hide();
        var language = WritingStyleRules.AppLanguage(
            LanguageStore.Read(), System.Globalization.CultureInfo.InstalledUICulture.TwoLetterISOLanguageName, L10n.Locales);
        _model.ChangeReplyDraft(new ReplyDraftIntent.Draft(_composition, SendingFrom(), instruction, language, replace));
    }

    // A draft that arrived after the person wrote meanwhile waits to be asked about, once.
    private async Task AskArrivedAsync(ComposerReplyDraft record)
    {
        if (!record.ReplaceArrived || _askingArrived || _model is null)
        {
            return;
        }
        _askingArrived = true;
        try
        {
            var replace = await AskReplaceAsync();
            _model.ChangeReplyDraft(new ReplyDraftIntent.AnswerArrived(_composition, replace));
        }
        finally
        {
            _askingArrived = false;
        }
    }

    private async Task<bool> AskReplaceAsync()
    {
        var dialog = new ContentDialog
        {
            Title = L10n.ComposerDraftReplaceTitle(),
            Content = new TextBlock { Text = L10n.ComposerDraftReplaceMessage(), TextWrapping = TextWrapping.Wrap },
            PrimaryButtonText = L10n.ComposerDraftReplace(),
            CloseButtonText = L10n.ActionCancel(),
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = XamlRoot,
        };
        return await DialogHelper.ShowAsync(dialog) == ContentDialogResult.Primary;
    }

    // The draft reaches this editor once per serial; the core's script says whether it goes in or
    // only follows the draft already in the body.
    private async Task RunDraftScriptAsync(ComposerReplyDraft record)
    {
        if (!_editorReady || record.EditorScript is not { } script || script.Serial == _draftScriptRun)
        {
            return;
        }
        _draftScriptRun = script.Serial;
        try
        {
            await _editor.RunAsync(script.Script);
        }
        catch (Exception ex)
        {
            Log.Warn($"composer: couldn't hand the draft to the editor ({ex.GetType().Name})");
        }
    }

    // The editor's messages, every one: only the core reads its reply-draft report.
    private void OnEditorPosted(string message)
    {
        if (_model is not null && !string.IsNullOrEmpty(_composition))
        {
            _model.ReplyDraftReport(_composition, message);
        }
    }

    // Send asks once while the checklist has open items; the core refuses the send meanwhile, so
    // the question is not the client's alone. Either answer is sent before anything is submitted.
    private async Task<bool> SendQuestionAllowsAsync()
    {
        if (_model?.ComposerReplyDraft(_composition, SendingFrom()) is not { SendQuestion: { } question })
        {
            return true;
        }
        var dialog = new ContentDialog
        {
            Title = L10n.ComposerSendOpenTitle(),
            Content = new TextBlock { Text = L10n.ComposerSendOpenMessage((int)question.Open), TextWrapping = TextWrapping.Wrap },
            PrimaryButtonText = L10n.ComposerSendAnyway(),
            CloseButtonText = L10n.ComposerKeepEditing(),
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = XamlRoot,
        };
        var send = await DialogHelper.ShowAsync(dialog) == ContentDialogResult.Primary;
        _model.ChangeReplyDraft(new ReplyDraftIntent.SendQuestionAnswered(_composition));
        return send;
    }

    // A failure's one remedy from its control: Try again reopens the input; a place in Settings, the
    // learning flow or the Allodia sign-in is opened over the composer, which keeps the draft and
    // sends nothing for it: its record follows on Surface.WritingStyle.
    private StackPanel? DraftRemedy(AiRemedyControl? remedy, string automationId)
    {
        if (remedy is not { } given)
        {
            return null;
        }
        return ControlComponents.Button(AiWords.Catalog().Of(given.Remedy), given.Control, automationId, () =>
        {
            _draftFlyout?.Hide();
            switch (AiRemedies.PlaceOf(given.Remedy))
            {
                case AiRemedyPlace.Retry:
                    OpenDraftFlyout();
                    break;
                case AiRemedyPlace.SignIn:
                    _model?.SignInToAllodia();
                    break;
                case AiRemedyPlace.Endpoint:
                    App.Shell?.ShowSettings("advanced");
                    break;
                case AiRemedyPlace.WritingStyle:
                    App.Shell?.ShowSettings("writing_style");
                    break;
                case AiRemedyPlace.Learn:
                    _model?.MoveLearning(new StyleLearningIntent.Open());
                    App.Shell?.ShowSettings("writing_style");
                    break;
            }
        });
    }

    // The notes Draft a reply adds under the action row: the message answered gone, a draft being
    // made with its Stop, one that did not come with its remedy, or the card. The one spinner while
    // drafting is the action row's control, which runs until the draft is in or stopped.
    private void DrawReplyDraftNotes(ComposerReplyDraft record)
    {
        ReplyDraftNotes.Children.Clear();
        if (record.HeldBy is { Failure: AiFailure.MessageGone } gone)
        {
            ReplyDraftNotes.Children.Add(NoteLine(AiWords.Catalog().Of(gone.Failure), "ComposerDraftGone"));
        }
        switch (record.State)
        {
            case ReplyDraftState.Drafting:
                var drafting = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10 };
                drafting.Children.Add(NoteLine(L10n.ComposerDrafting(), "ComposerDrafting"));
                drafting.Children.Add(ControlComponents.Button(L10n.ComposerDraftStop(), record.Stop, "ComposerDraftStop", () =>
                    _model?.ChangeReplyDraft(new ReplyDraftIntent.Stop(_composition))));
                ReplyDraftNotes.Children.Add(drafting);
                break;
            case ReplyDraftState.Failed failed:
                var bar = new InfoBar
                {
                    Severity = InfoBarSeverity.Error,
                    IsOpen = true,
                    IsClosable = false,
                    Message = AiWords.Catalog().Of(failed.Failure),
                };
                AutomationProperties.SetAutomationId(bar, "ComposerDraftFailed");
                if (DraftRemedy(failed.Remedy, "ComposerDraftRemedy") is { } remedy)
                {
                    bar.Content = new Border { Padding = new Thickness(0, 0, 0, 12), Child = remedy };
                }
                ReplyDraftNotes.Children.Add(bar);
                break;
            case ReplyDraftState.Drafted drafted:
                ReplyDraftNotes.Children.Add(DraftCard(drafted));
                break;
        }
        ReplyDraftNotes.Visibility = ReplyDraftNotes.Children.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    private static TextBlock NoteLine(string text, string automationId)
    {
        var line = new TextBlock
        {
            Text = text,
            TextWrapping = TextWrapping.Wrap,
            VerticalAlignment = VerticalAlignment.Center,
            Style = (Style)Application.Current.Resources["BodyTextBlockStyle"],
        };
        AutomationProperties.SetAutomationId(line, automationId);
        return line;
    }

    // The card: what the message asks, then what is left to do. The person's items are ticks; a gap,
    // which has no tick, is a mark the reply decides. It is drawn beside the editor, so it is never
    // part of the mail.
    private Border DraftCard(ReplyDraftState.Drafted drafted)
    {
        var body = new StackPanel { Spacing = 6 };
        if (drafted.Summary.Length > 0)
        {
            body.Children.Add(CardHeading(L10n.ComposerDraftSummaryHeading()));
            var summary = new TextBlock { Text = drafted.Summary, TextWrapping = TextWrapping.Wrap };
            AutomationProperties.SetAutomationId(summary, "ComposerDraftSummary");
            body.Children.Add(summary);
        }
        if (drafted.Checklist.Length > 0)
        {
            var heading = CardHeading(L10n.ComposerChecklistHeading());
            heading.Margin = new Thickness(0, drafted.Summary.Length > 0 ? 8 : 0, 0, 0);
            body.Children.Add(heading);
            foreach (var item in drafted.Checklist)
            {
                var index = item.Index;
                var text = Words.Of(item);
                body.Children.Add(item.Tick is { } tick
                    ? ControlComponents.CheckBox(text, item.Done, tick, $"ComposerDraftItem_{index}", done =>
                        _model?.ChangeReplyDraft(new ReplyDraftIntent.Tick(_composition, index, done)))
                    : new StateMark(text, item.Done, $"ComposerDraftItem_{index}"));
            }
        }
        var card = new Border
        {
            Background = (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources["CardBackgroundFillColorDefaultBrush"],
            BorderBrush = (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources["CardStrokeColorDefaultBrush"],
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(14, 10, 14, 12),
            Child = body,
        };
        AutomationProperties.SetAutomationId(card, "ComposerDraftCard");
        return card;
    }

    private static TextBlock CardHeading(string text)
    {
        var heading = new TextBlock { Text = text, FontWeight = FontWeights.SemiBold };
        AutomationProperties.SetHeadingLevel(heading, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level3);
        return heading;
    }
}
