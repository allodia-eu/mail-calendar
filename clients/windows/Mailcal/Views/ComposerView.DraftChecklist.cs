// The card a drafted reply brings (docs/ai.md, "Where they show"): what the message asks, and what
// is left to do, between the editor and the action row. It is WinUI beside the editor, so it is
// never part of the mail. The rules are DraftChecklist, where Mailcal.Tests pins them; what is here
// is drawing it, following the editor while a placeholder is open, and the one question Send asks.

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

public sealed partial class ComposerView
{
    private readonly DraftChecklist _checklist = new();

    // One checkbox per item, in the checklist's order, so a tick can be drawn without redrawing
    // the card under the person's pointer.
    private readonly List<(CheckBox Box, TextBlock Text)> _checklistRows = [];

    // About once a second while a fill-in item is open; stopped with the composer.
    private DispatcherQueueTimer? _placeholderTimer;
    private bool _readingPlaceholders;
    private bool _checklistClosed;

    // Keeps the ticks drawn from the checklist from being taken for the person's own.
    private bool _syncingTicks;

    // The card's scrolling body, whose cap follows the room the editor leaves it.
    private ScrollViewer? _cardBody;

    // A new card has not been laid out yet, so whether it opens folded is still to be decided.
    private bool _cardUnfitted;

    // A draft went in: its card replaces an earlier draft's, or goes when it has nothing to say.
    private void ShowDraftCard(DraftReply draft)
    {
        _checklist.Show(draft.Summary, draft.Tasks, _attachments.Count);
        DrawDraftCard();
        FollowPlaceholders();
    }

    private void DrawDraftCard()
    {
        _checklistRows.Clear();
        _cardBody = null;
        if (_checklist.IsEmpty)
        {
            DraftCard.Visibility = Visibility.Collapsed;
            DraftCard.Content = null;
            return;
        }
        var summary = _checklist.Summary;
        DraftCard.Header = summary.Length > 0 ? L10n.ComposerDraftSummaryHeading() : L10n.ComposerChecklistHeading();
        var body = new StackPanel { Spacing = 6 };
        if (summary.Length > 0)
        {
            body.Children.Add(new TextBlock { Text = summary, TextWrapping = TextWrapping.Wrap });
            if (_checklist.Items.Count > 0)
            {
                var heading = new TextBlock
                {
                    Text = L10n.ComposerChecklistHeading(),
                    FontWeight = FontWeights.SemiBold,
                    Margin = new Thickness(0, 6, 0, 0),
                };
                AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level3);
                body.Children.Add(heading);
            }
        }
        foreach (var item in _checklist.Items)
        {
            body.Children.Add(ChecklistRow(item));
        }
        _cardBody = new ScrollViewer { Content = body, MaxHeight = DraftChecklist.CardBodyMax };
        _cardBody.SizeChanged += (_, _) => FitDraftCard();
        DraftCard.Content = _cardBody;
        DraftCard.IsExpanded = true;
        DraftCard.Visibility = Visibility.Visible;
        _cardUnfitted = true;
        SyncTicks();
    }

    private void OnEditorFrameSizeChanged(object sender, SizeChangedEventArgs e) => FitDraftCard();

    // Keeps room for the reply: a new card opens folded where both floors do not fit, and an open
    // one has its body capped (DraftChecklist). A folded card takes no room, so it is left alone,
    // and folding or opening it afterwards is the person's.
    private void FitDraftCard()
    {
        if (_cardBody is not { } body || DraftCard.Visibility != Visibility.Visible || !DraftCard.IsExpanded)
        {
            return;
        }
        var room = EditorFrame.ActualHeight + body.ActualHeight;
        if (_cardUnfitted)
        {
            // The editor can report its new size before the new body has one, which would read as
            // no room at all; the body's own size change comes once it is laid out.
            if (body.ActualHeight <= 0)
            {
                return;
            }
            _cardUnfitted = false;
            if (DraftChecklist.CardOpensFolded(room))
            {
                DraftCard.IsExpanded = false;
                return;
            }
        }
        var ceiling = DraftChecklist.CardBodyCeiling(room);
        if (Math.Abs(body.MaxHeight - ceiling) >= 1)
        {
            body.MaxHeight = ceiling;
        }
    }

    // A checkbox, an icon by kind, and the text. A fill-in item ticks itself as the reply changes,
    // so its box takes no pointer and no focus; it is not disabled, which would grey its words.
    private CheckBox ChecklistRow(DraftChecklistItem item)
    {
        var text = new TextBlock
        {
            Text = item.Kind == DraftTaskKind.FillIn ? L10n.ComposerTaskFillIn(item.Text) : item.Text,
            TextWrapping = TextWrapping.Wrap,
            VerticalAlignment = VerticalAlignment.Center,
        };
        var icon = new FontIcon
        {
            Glyph = item.Kind switch
            {
                DraftTaskKind.FillIn => "\uE70F",
                DraftTaskKind.Attach => "\uE723",
                _ => "\uE9D5",
            },
            FontSize = 14,
            Width = 18,
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetAccessibilityView(icon, AccessibilityView.Raw);
        var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        content.Children.Add(icon);
        content.Children.Add(text);
        var byHand = item.Kind != DraftTaskKind.FillIn;
        var box = new CheckBox
        {
            Content = content,
            IsHitTestVisible = byHand,
            IsTabStop = byHand,
            MinHeight = 28,
            Padding = new Thickness(8, 4, 0, 4),
        };
        // A box derives no name from a panel of text, and the kind's icon says what the words are.
        AutomationProperties.SetName(box, item.Kind switch
        {
            DraftTaskKind.FillIn => text.Text,
            DraftTaskKind.Attach => $"{L10n.A11yTaskAttach()}, {item.Text}",
            _ => $"{L10n.A11yTaskDo()}, {item.Text}",
        });
        var id = item.Id;
        box.Checked += (_, _) => OnHandTick(id);
        box.Unchecked += (_, _) => OnHandTick(id);
        _checklistRows.Add((box, text));
        return box;
    }

    // The person's tick. One on a fill-in item, which only an assistive technology can still reach,
    // is drawn back as the reply has it.
    private void OnHandTick(int id)
    {
        if (_syncingTicks)
        {
            return;
        }
        _checklist.Toggle(id);
        SyncTicks();
    }

    // Draws every item's tick as the checklist holds it: a done item is struck through and quieter.
    private void SyncTicks()
    {
        _syncingTicks = true;
        for (var index = 0; index < _checklistRows.Count && index < _checklist.Items.Count; index++)
        {
            var ticked = _checklist.Items[index].Ticked;
            var (box, text) = _checklistRows[index];
            box.IsChecked = ticked;
            text.TextDecorations = ticked
                ? Windows.UI.Text.TextDecorations.Strikethrough
                : Windows.UI.Text.TextDecorations.None;
            text.Opacity = ticked ? 0.6 : 1;
        }
        _syncingTicks = false;
    }

    // The attach items tick themselves once there is a new file for every one of them.
    private void OnAttachmentsChanged()
    {
        _checklist.AttachmentsChanged(_attachments.Count);
        SyncTicks();
    }

    // Asks the editor about once a second while a fill-in item is open, and stops once none is.
    private void FollowPlaceholders()
    {
        if (_checklistClosed || !_checklist.AwaitsPlaceholders)
        {
            _placeholderTimer?.Stop();
            return;
        }
        if (_placeholderTimer is null)
        {
            _placeholderTimer = DispatcherQueue.CreateTimer();
            _placeholderTimer.Interval = TimeSpan.FromSeconds(1);
            _placeholderTimer.IsRepeating = true;
            _placeholderTimer.Tick += (_, _) => _ = ReadPlaceholdersAsync(fresh: false);
        }
        if (!_placeholderTimer.IsRunning)
        {
            _placeholderTimer.Start();
        }
    }

    // Reads which placeholders are still in the reply and ticks the others. A tick that finds a
    // read still under way skips its turn; Send reads regardless, since it must not act on an old
    // answer.
    private async Task ReadPlaceholdersAsync(bool fresh)
    {
        var placeholders = _checklist.Placeholders;
        if (_checklistClosed || !_editorReady || placeholders.Length == 0 || (_readingPlaceholders && !fresh))
        {
            return;
        }
        var draft = _checklist.Draft;
        _readingPlaceholders = true;
        try
        {
            var left = DraftChecklist.ReadLeft(await _editor.EvaluateAsync(DraftChecklist.LeftScript(placeholders)));
            if (left is not null && _checklist.PlaceholdersLeft(left, draft))
            {
                SyncTicks();
            }
        }
        catch (Exception ex)
        {
            // The exception's kind only: the placeholders are words of the person's mail.
            Log.Warn($"composer: couldn't read the draft's placeholders ({ex.GetType().Name})");
        }
        finally
        {
            _readingPlaceholders = false;
        }
        FollowPlaceholders();
    }

    // Send asks once while an item is open, with the placeholders read afresh, and never blocks:
    // either answer ends the asking for this composer. True when the send goes ahead.
    private async Task<bool> ConfirmOpenItemsAsync()
    {
        if (_checklist.HasAsked || _checklist.Items.Count == 0)
        {
            return true;
        }
        await ReadPlaceholdersAsync(fresh: true);
        if (!_checklist.AsksBeforeSend)
        {
            return true;
        }
        // Another dialog is up, so this one would be dropped unasked: stay, and ask next time.
        if (DialogHelper.IsShowing)
        {
            return false;
        }
        var answer = await DialogHelper.ConfirmAsync(
            XamlRoot,
            L10n.ComposerSendOpenTitle(),
            L10n.ComposerSendOpenMessage(_checklist.OpenCount),
            L10n.ComposerSendAnyway(),
            L10n.ComposerKeepEditing());
        _checklist.SendAsked();
        return answer == ContentDialogResult.Primary;
    }

    // The composer is closing: nothing is asked of its editor again.
    private void CloseDraftCard()
    {
        _checklistClosed = true;
        _placeholderTimer?.Stop();
    }
}
