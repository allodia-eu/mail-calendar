// The window's one notice bar (docs/notices.md). Which notice it draws and how many stand behind
// it is NoticeRules; this half turns that into an InfoBar, a flyout and an announcement.

using System.ComponentModel;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Views;

/// <summary>One InfoBar for the notice that belongs to the screen on show, and a flyout for the
/// rest.</summary>
public sealed partial class NoticeBar : UserControl
{
    private MailboxModel? _model;
    private Notice? _shown;
    private readonly NoticeAnnouncements _announced = new();

    /// <summary>Initialises the control.</summary>
    public NoticeBar() => InitializeComponent();

    /// <summary>Draws <paramref name="model"/>'s notices from now on.</summary>
    public void Init(MailboxModel model)
    {
        _model = model;
        model.NoticesChanged += OnNoticesChanged;
        model.PropertyChanged += OnModelChanged;
        Draw();
    }

    private void OnNoticesChanged()
    {
        Draw();
        Announce();
    }

    private void OnModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(MailboxModel.Destination))
        {
            Draw();
        }
    }

    private void Draw()
    {
        if (_model is null)
        {
            return;
        }
        var notices = _model.Notices;
        var screen = NoticeRules.ScreenOf(_model.Destination);
        _shown = NoticeRules.Prominent(notices, screen);
        if (_shown is not { } notice)
        {
            Bar.IsOpen = false;
            Bar.Visibility = Visibility.Collapsed;
            MoreFlyout.Hide();
            return;
        }
        Bar.Severity = SeverityOf(notice.Severity);
        Bar.Title = MailboxModel.Words(notice.Title);
        Bar.Message = Beneath(notice) ?? string.Empty;
        Bar.IsClosable = notice.Dismissible;
        var question = NoticeRules.OffersSecond(notice);
        if (NoticeRules.Offers(notice) && !question)
        {
            var label = MailboxModel.Words(notice.ActionLabel!);
            ActionLabel.Text = label;
            // Its content is not a string, so the name is given.
            AutomationProperties.SetName(ActionButton, label);
            ActionButton.IsEnabled = !notice.Running;
            ActionRunning.IsActive = notice.Running;
            ActionRunning.Visibility = notice.Running ? Visibility.Visible : Visibility.Collapsed;
            Bar.ActionButton = ActionButton;
        }
        else
        {
            Bar.ActionButton = null;
        }
        if (question)
        {
            FirstAnswer.Content = MailboxModel.Words(notice.ActionLabel!);
            SecondAnswer.Content = MailboxModel.Words(notice.SecondActionLabel!);
            // An answer the core is carrying out holds both, the spinner beside the first.
            FirstAnswer.IsEnabled = SecondAnswer.IsEnabled = !notice.Running;
            AnswerRunning.IsActive = notice.Running;
            AnswerRunning.Visibility = notice.Running ? Visibility.Visible : Visibility.Collapsed;
        }
        Answers.Visibility = question ? Visibility.Visible : Visibility.Collapsed;
        var more = NoticeRules.More(notices, screen);
        MoreButton.Content = L10n.NoticesMore(more);
        MoreButton.Visibility = more > 0 ? Visibility.Visible : Visibility.Collapsed;
        Bar.Visibility = Visibility.Visible;
        Bar.IsOpen = true;
    }

    /// <summary>What is said beneath a notice's sentence: that its remedy did not work, which
    /// stands until the notice goes (rule 10), or else its detail.</summary>
    internal static string? Beneath(Notice notice) =>
        notice.RemedyFailed ? L10n.NoticesRemedyFailed()
        : notice.Detail is { } detail ? MailboxModel.Words(detail)
        : null;

    internal static InfoBarSeverity SeverityOf(NoticeSeverity severity) => severity switch
    {
        NoticeSeverity.Blocked => InfoBarSeverity.Error,
        NoticeSeverity.Degraded => InfoBarSeverity.Warning,
        _ => InfoBarSeverity.Informational,
    };

    /// <summary>Says each notice that has just arrived to assistive technology, once (rule 12).</summary>
    private void Announce()
    {
        if (_model is null)
        {
            return;
        }
        var arrived = _announced.Arrived(_model.Notices);
        if (arrived.Count == 0)
        {
            return;
        }
        var peer = FrameworkElementAutomationPeer.FromElement(this)
            ?? FrameworkElementAutomationPeer.CreatePeerForElement(this);
        foreach (var notice in arrived)
        {
            peer?.RaiseNotificationEvent(
                AutomationNotificationKind.Other,
                AutomationNotificationProcessing.ImportantAll,
                MailboxModel.Words(notice.Title),
                "notice");
        }
    }

    private void OnAction(object sender, RoutedEventArgs e) => Run(notice => notice.Action);

    private void OnFirstAnswer(object sender, RoutedEventArgs e) => Run(notice => notice.Action);

    private void OnSecondAnswer(object sender, RoutedEventArgs e) => Run(notice => notice.SecondAction);

    private void Run(Func<Notice, NoticeAction?> which)
    {
        if (_shown is { } notice && which(notice) is { } action)
        {
            _model?.RunNoticeAction(notice, action);
        }
    }

    private void OnClose(InfoBar sender, object args)
    {
        if (_shown is { } notice)
        {
            _model?.DismissNotice(notice);
        }
    }

    /// <summary>Lists every notice in force, each with its own action, built as it opens so it
    /// is never a snapshot behind.</summary>
    private void OnMoreOpening(object? sender, object e)
    {
        MoreList.Children.Clear();
        foreach (var notice in _model?.Notices ?? [])
        {
            MoreList.Children.Add(Row(notice));
        }
    }

    private UIElement Row(Notice notice)
    {
        var tone = NoticeRules.ToneOf(notice.Severity);
        var icon = new FontIcon
        {
            Glyph = SidebarMark.GlyphFor(tone),
            FontSize = 16,
            Foreground = ToneBrush(tone),
            VerticalAlignment = VerticalAlignment.Top,
            Margin = new Thickness(0, 2, 0, 0),
        };
        AutomationProperties.SetAccessibilityView(icon, AccessibilityView.Raw);
        var text = new StackPanel { Spacing = 4 };
        var title = MailboxModel.Words(notice.Title);
        text.Children.Add(new TextBlock { Text = title, TextWrapping = TextWrapping.Wrap });
        if (Beneath(notice) is { } beneath)
        {
            text.Children.Add(new TextBlock
            {
                Text = beneath,
                TextWrapping = TextWrapping.Wrap,
                Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
                Foreground = (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"],
            });
        }
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        if (NoticeRules.Offers(notice))
        {
            buttons.Children.Add(ListAnswer(
                notice, notice.Action!.Value, notice.ActionLabel!, title, "NoticeListAction", first: NoticeRules.OffersSecond(notice)));
        }
        if (NoticeRules.OffersSecond(notice))
        {
            buttons.Children.Add(ListAnswer(
                notice, notice.SecondAction!.Value, notice.SecondActionLabel!, title, "NoticeListSecondAction", first: false));
        }
        if (notice.Running)
        {
            var ring = new ProgressRing { IsActive = true, Width = 16, Height = 16 };
            AutomationProperties.SetAutomationId(ring, "NoticeListAction_Running");
            buttons.Children.Add(ring);
        }
        if (notice.Dismissible)
        {
            var close = new Button { Content = L10n.NoticesDismiss() };
            AutomationProperties.SetAutomationId(close, "NoticeListDismiss");
            AutomationProperties.SetName(close, $"{L10n.NoticesDismiss()}: {title}");
            close.Click += (_, _) =>
            {
                MoreFlyout.Hide();
                _model?.DismissNotice(notice);
            };
            buttons.Children.Add(close);
        }
        if (buttons.Children.Count > 0)
        {
            text.Children.Add(buttons);
        }
        var row = new Grid { ColumnSpacing = 12 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        Grid.SetColumn(text, 1);
        row.Children.Add(icon);
        row.Children.Add(text);
        AutomationProperties.SetAutomationId(row, "NoticeListRow");
        return row;
    }

    /// <summary>One of a listed notice's buttons, named with the notice it acts on, held while an
    /// answer runs. A question's first answer is the accent one, as in the bar.</summary>
    private Button ListAnswer(Notice notice, NoticeAction action, NoticeText label, string title, string automationId, bool first)
    {
        var words = MailboxModel.Words(label);
        var button = new Button { Content = words, IsEnabled = !notice.Running };
        if (first)
        {
            button.Style = (Style)Application.Current.Resources["AccentButtonStyle"];
        }
        AutomationProperties.SetAutomationId(button, automationId);
        AutomationProperties.SetName(button, $"{words}: {title}");
        button.Click += (_, _) =>
        {
            MoreFlyout.Hide();
            _model?.RunNoticeAction(notice, action);
        };
        return button;
    }

    /// <summary>The colour of a notice's glyph: the system's critical, caution and neutral fills,
    /// so it follows the theme and high contrast. Never the only signal: the sentence is beside it.</summary>
    internal static Brush ToneBrush(SidebarMarkTone tone) => (Brush)Application.Current.Resources[tone switch
    {
        SidebarMarkTone.Blocked => "SystemFillColorCriticalBrush",
        SidebarMarkTone.Degraded => "SystemFillColorCautionBrush",
        _ => "TextFillColorSecondaryBrush",
    }];
}
