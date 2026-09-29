// Settings → Writing style (docs/ai.md, docs/settings.md slot 7): the learned styles, each
// account's style, and the credits line when requests go through Allodia's relay. The category is
// in the source list only while the core says AI has somewhere to go (WritingStyleSnapshot.Route),
// and it comes and goes with that while the dialog is open.
//
// Everything renders inside this dialog's detail panel, as the signature editor does: WinUI forbids
// a nested ContentDialog, so the learn sheet (SettingsDialog.LearnStyle.cs) and the reveal
// (SettingsDialog.StyleReveal.cs) replace the panel, and forgetting a style confirms in place.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // What the category shows. The library is where it rests; every other screen goes back to it.
    private enum StyleScreen
    {
        Library,
        Learn,
        Learning,
        Failed,
        Style,
    }

    private StyleScreen _styleScreen = StyleScreen.Library;
    private LearnSheet? _learnSheet;
    private AiFailure? _learnFailure;
    private string? _openStyle;

    // Whether the Allodia category has asked the relay for the balance since it was opened.
    private bool _aiBalanceAsked;

    private string? CurrentCategory => (_categories.SelectedItem as ListViewItem)?.Tag as string;

    // Leaving the category abandons the sheet or the reveal, as leaving Signatures abandons its
    // editor. A learning run already started goes on in the core, and the category shows its
    // progress when the person comes back.
    private void LeaveWritingStyle(string tag)
    {
        if (tag != "writing_style")
        {
            _styleScreen = StyleScreen.Library;
            _learnSheet = null;
            _learnFrame = null;
            _learnReport = null;
            _learnFailure = null;
            _openStyle = null;
            _reveal = null;
        }
        if (tag != "allodia")
        {
            _aiBalanceAsked = false;
        }
    }

    private UIElement BuildWritingStyle()
    {
        var snapshot = _model.WritingStyles;
        // A run in progress is what the category shows until it ends, including one started before
        // this dialog opened.
        if (_styleScreen == StyleScreen.Learning
            || (_styleScreen == StyleScreen.Library && snapshot.Learning is not null))
        {
            return BuildLearning(snapshot.Learning);
        }
        return _styleScreen switch
        {
            StyleScreen.Learn when _learnSheet is { } sheet => BuildLearnSheet(sheet, snapshot),
            StyleScreen.Failed when _learnFailure is { } failure => BuildLearnFailed(failure),
            StyleScreen.Style when _openStyle is { } id && _model.WritingStyleDetailOf(id) is { } detail =>
                BuildStyleReveal(detail),
            _ => BuildStyleLibrary(snapshot),
        };
    }

    private UIElement BuildStyleLibrary(WritingStyleSnapshot snapshot)
    {
        // Counts only, never a name (docs/logging.md), for the reason the Signatures panel logs.
        Log.Info($"settings: writing style panel, {snapshot.Styles.Length} learned, {snapshot.Accounts.Length} account(s)");
        var panel = new StackPanel { Spacing = 20 };
        var top = new StackPanel { Spacing = 8 };
        if (snapshot.Refused is { } refused)
        {
            // The gate's verdict takes the Learn button's place: nothing it would send may leave.
            top.Children.Add(Warning(WritingStyleText.Of(AiFailure.Refusal(refused.Mode))));
        }
        else
        {
            var learn = new Button
            {
                Content = L10n.WritingStyleLearn(),
                IsEnabled = snapshot.Accounts.Length > 0,
            };
            learn.Click += (_, _) => OpenLearnSheet(snapshot.Accounts[0].AccountId, snapshot.Accounts.Length);
            top.Children.Add(learn);
        }
        if (snapshot.Route == AiRoute.Relay && snapshot.Balance is { } balance)
        {
            top.Children.Add(Description(WritingStyleText.Credits(balance, _model.ActiveZone)));
        }
        panel.Children.Add(Group(L10n.SettingsCategoryWritingStyle(), L10n.WritingStyleIntro(), top));
        panel.Children.Add(StyleList(snapshot.Styles));
        var accounts = new StackPanel { Spacing = 6 };
        accounts.Children.Add(Heading(L10n.WritingStyleAccountsHeading()));
        accounts.Children.Add(AccountStylePickers(snapshot));
        panel.Children.Add(accounts);
        return panel;
    }

    // Every learned style, each opening the style itself.
    private UIElement StyleList(IReadOnlyList<WritingStyleRow> styles)
    {
        var panel = new StackPanel { Spacing = 4 };
        if (styles.Count == 0)
        {
            panel.Children.Add(Description(L10n.WritingStyleEmpty()));
        }
        foreach (var style in styles)
        {
            panel.Children.Add(StyleRow(style));
        }
        return panel;
    }

    // One library row: the name, what it was learned from, and its languages. The whole row opens
    // the style, where the one destructive action confirms first, so a stray click costs nothing.
    private Button StyleRow(WritingStyleRow style)
    {
        var text = new StackPanel { Spacing = 2 };
        var name = Heading(style.Name);
        name.TextTrimming = TextTrimming.CharacterEllipsis;
        text.Children.Add(name);
        // Zero when the style came from another device and nobody knows when it was learned.
        if (style.LearnedAt > 0)
        {
            text.Children.Add(Description(L10n.WritingStyleLearnedFrom(
                (int)style.Messages, WritingStyleText.Date(style.LearnedAt, _model.ActiveZone))));
        }
        if (style.Languages.Length > 0)
        {
            text.Children.Add(Description(
                L10n.WritingStyleLanguages(WritingStyleText.Languages(style.Languages))));
        }
        var row = new Button
        {
            Content = text,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Left,
            Padding = new Thickness(10, 8, 10, 8),
        };
        // A Button derives no name from a panel of text, and a row that announces only "button"
        // does not say which style it opens.
        AutomationProperties.SetName(row, style.Name);
        row.Click += (_, _) => OpenStyle(style.Id);
        return row;
    }

    // For each account, the style it drafts in, or None. The label is the address, above the
    // control, as each signature slot's is.
    private UIElement AccountStylePickers(WritingStyleSnapshot snapshot)
    {
        if (snapshot.Accounts.Length == 0)
        {
            return Description(L10n.SettingsAccountsEmpty());
        }
        var panel = new StackPanel { Spacing = 12 };
        foreach (var account in snapshot.Accounts)
        {
            var options = new List<StyleOption> { new(null, L10n.WritingStyleNone()) };
            options.AddRange(snapshot.Styles.Select(style => new StyleOption(style.Id, style.Name)));
            var box = new ComboBox
            {
                Header = account.Email,
                MinWidth = 240,
                ItemsSource = options,
                SelectedItem = options.FirstOrDefault(option => option.Id == account.Style) ?? options[0],
            };
            var id = account.AccountId;
            box.SelectionChanged += (_, _) =>
            {
                if (!_rebuilding && box.SelectedItem is StyleOption option)
                {
                    _model.SetAccountWritingStyle(id, option.Id);
                }
            };
            panel.Children.Add(box);
        }
        return panel;
    }

    // The credits line under the Allodia account, when somebody is signed in and the relay has
    // reported a balance. Opening the category asks for a fresh one when requests go through the
    // relay; the answer arrives as a Writing style signal, and a failure leaves the line it had.
    private TextBlock? AllodiaCredits()
    {
        if (_model.SignedInAllodiaAccount() is null)
        {
            return null;
        }
        var snapshot = _model.WritingStyles;
        if (snapshot.Route == AiRoute.Relay && !_aiBalanceAsked)
        {
            _aiBalanceAsked = true;
            _ = _model.RefreshAiBalanceAsync();
        }
        return snapshot.Balance is { } balance
            ? Description(WritingStyleText.Credits(balance, _model.ActiveZone))
            : null;
    }

    // A signal from the core: the library, an assignment, the route, the balance or a run's
    // progress moved. Redrawn only where nothing is being typed: the learn sheet and the reveal hold
    // the person's input, and a redraw would drop it.
    private void OnWritingStylesChanged()
    {
        SyncCategories();
        var tag = CurrentCategory;
        if ((tag == "writing_style" && _styleScreen is StyleScreen.Library or StyleScreen.Learning)
            || tag == "allodia")
        {
            Apply(() => { });
        }
    }

    // Adds or drops the Writing style category when the route appears or goes: an own endpoint
    // saved or removed under Advanced, or the relay's entitlement arriving.
    private void SyncCategories()
    {
        var wanted = Categories();
        var shown = _categories.Items.OfType<ListViewItem>().Select(item => item.Tag as string);
        if (shown.SequenceEqual(wanted.Select(entry => entry.Tag)))
        {
            return;
        }
        var current = CurrentCategory;
        var index = Array.FindIndex(wanted, entry => entry.Tag == current);
        _rebuilding = true;
        _categories.Items.Clear();
        foreach (var (tag, label) in wanted)
        {
            _categories.Items.Add(new ListViewItem { Content = label, Tag = tag });
        }
        _categories.SelectedIndex = index >= 0 ? index : Array.FindIndex(wanted, entry => entry.Tag == "general");
        _rebuilding = false;
        // The category on screen went away with the route, so the panel follows the selection.
        if (index < 0)
        {
            ShowCategory("general");
        }
    }

    private void OpenStyle(string id) => Apply(() =>
    {
        _openStyle = id;
        _reveal = null;
        _styleScreen = StyleScreen.Style;
    });

    private TextBlock Warning(string text)
    {
        var line = Description(text);
        line.Opacity = 1;
        line.Foreground = _brushes.Of(ThemePalette.Critical);
        return line;
    }

    private static TextBlock Line(string text) => new() { Text = text, TextWrapping = TextWrapping.Wrap };

    // One entry in an account's picker; ToString is the label so the ComboBox shows it directly. A
    // null Id is None, a choice the person can make.
    private sealed record StyleOption(string? Id, string Label)
    {
        public override string ToString() => Label;
    }
}
