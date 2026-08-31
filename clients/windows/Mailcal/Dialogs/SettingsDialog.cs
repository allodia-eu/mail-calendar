// The unified Settings dialog, the Windows twin of macOS's SettingsView and Android's
// SettingsScreen. A categorised editor (a source-list of General / Reading / Composing /
// Accounts / Advanced beside a detail panel) that consolidates settings which used to be
// scattered across the toolbars and the calendar header: language, time zone, conversation
// grouping, the default quote style, per-account fetch depth + sync behaviour, and the
// destructive database reset. Built imperatively in code-behind (no XAML data-binding) so the
// control tree is deterministic; state lives in Rust (the snapshot the model projects) and each
// change forwards to the core, which re-signals the settings surface. The one host-owned
// setting is the UI language, which needs a restart to re-resolve already-loaded x:Bind text.

using System;
using System.Linq;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>A modal, categorised app-settings editor.</summary>
public sealed partial class SettingsDialog : ContentDialog
{
    private readonly MailboxModel _model;

    // The category source-list and the detail panel its selection fills. The detail is rebuilt
    // per category (and, within Accounts, after a strategy/folder change) so it always mirrors
    // the core.
    private readonly ListView _categories = new() { SelectionMode = ListViewSelectionMode.Single };
    private readonly StackPanel _detail = new() { Spacing = 16 };

    // Guards the programmatic IsChecked / SelectedItem / SelectedIndex assignments during a
    // (re)build from firing the change handlers (which would re-enter the build), the same
    // pattern the mail list's mode toggle and the old sync dialog used.
    private bool _rebuilding;

    // The language restart hint, revealed once the user changes the UI language (already-loaded
    // text only re-resolves on the next launch). Held so a rebuild of the General panel can keep
    // it open if the user already switched.
    private bool _languageChanged;

    /// <summary>Builds the dialog over the shared model, opened on <paramref name="category"/>,
    /// a tag from <see cref="Categories"/>. Only the screenshot driver passes one; the app itself
    /// always opens on General, which is not the first category in a build that carries the
    /// Allodia route.</summary>
    public SettingsDialog(MailboxModel model, string category = "general")
    {
        _model = model;
        Title = L10n.SettingsTitle();
        CloseButtonText = L10n.ActionDone();
        DefaultButton = ContentDialogButton.Close;
        // The default ContentDialog max width (~548) would clip the sidebar+detail layout, so
        // widen this dialog to fit the 680-wide content.
        Resources["ContentDialogMaxWidth"] = 760.0;

        foreach (var (tag, label) in Categories())
        {
            _categories.Items.Add(new ListViewItem { Content = label, Tag = tag });
        }
        _categories.SelectionChanged += (_, _) =>
        {
            if (!_rebuilding && (_categories.SelectedItem as ListViewItem)?.Tag is string tag)
            {
                ShowCategory(tag);
            }
        };

        var root = new Grid { Width = 680, Height = 500, ColumnSpacing = 16 };
        root.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(180) });
        root.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        Grid.SetColumn(_categories, 0);
        var scroller = new ScrollViewer { Content = _detail };
        Grid.SetColumn(scroller, 1);
        root.Children.Add(_categories);
        root.Children.Add(scroller);
        Content = root;

        // Dismissing the dialog leaves the detail panel built, so a signature editor open at that
        // moment would keep its WebView2 alive past the dialog.
        Closed += (_, _) => CloseSignatureEditor();

        var index = Array.FindIndex(Categories(), entry => entry.Tag == category);
        _rebuilding = true;
        _categories.SelectedIndex = index < 0 ? 0 : index;
        _rebuilding = false;
        ShowCategory(index < 0 ? "general" : category);
    }

    // The category rows, in display order, tag (used to dispatch) + localised label.
    //
    // Allodia is first and is dropped whole in a build carrying no registration: a row that opens
    // an empty panel reads as a broken panel rather than as a build without a route, and it is what
    // every build from source would show. The category and the group inside it share one string,
    // because the product's own name belongs in one place.
    private static (string Tag, string Label)[] Categories() =>
        MailcalBindingsMethods.AllodiaSignInAvailable()
            ? new[] { ("allodia", L10n.SettingsAllodiaHeading()) }.Concat(MailCategories()).ToArray()
            : MailCategories();

    private static (string Tag, string Label)[] MailCategories() => new[]
    {
        ("general", L10n.SettingsCategoryGeneral()),
        ("calendar", L10n.SettingsCategoryCalendar()),
        ("reading", L10n.SettingsCategoryReading()),
        ("composing", L10n.SettingsCategoryComposing()),
        // Its own category, not a sub-screen of Composing: a signature is a standalone entity reused
        // across accounts, and "Settings → Signatures" is the path people already look for
        // (docs/settings.md slot 5). Notifications, slot 6, is mobile-only and absent here.
        ("signatures", L10n.SettingsCategorySignatures()),
        ("privacy", L10n.SettingsCategoryPrivacy()),
        ("accounts", L10n.SettingsCategoryAccounts()),
        ("advanced", L10n.SettingsCategoryAdvanced()),
        ("diagnostics", L10n.SettingsCategoryDiagnostics()),
        ("about", L10n.SettingsCategoryAbout()),
    };

    // Rebuilds the detail panel for the selected category.
    private void ShowCategory(string tag)
    {
        _rebuilding = true;
        // The Signatures editor hosts a WebView2, and dropping one from the tree does not release
        // it. Every rebuild replaces the panel wholesale, so the outgoing host goes here, including
        // when the rebuild is what closes the editor.
        CloseSignatureEditor();
        // Leaving the category abandons whatever it was mid-way through, so coming back shows the
        // library rather than an editor over a signature the user walked away from.
        if (tag != "signatures")
        {
            _editingSignature = null;
            _deletingSignature = null;
        }
        _detail.Children.Clear();
        _detail.Children.Add(tag switch
        {
            "allodia" => BuildAllodiaAccount()!,
            "calendar" => BuildCalendar(),
            "reading" => BuildReading(),
            "composing" => BuildComposing(),
            "signatures" => BuildSignatures(),
            "privacy" => BuildPrivacy(),
            "accounts" => BuildAccounts(),
            "advanced" => BuildAdvanced(),
            "diagnostics" => BuildDiagnostics(),
            "about" => BuildAbout(),
            _ => BuildGeneral(),
        });
        _rebuilding = false;
    }

    // A labelled settings group: a heading, a one-line description, and its control(s). Mirrors
    // the macOS SettingsView's settingsGroup GroupBox.
    private static UIElement Group(string heading, string description, UIElement content)
    {
        var panel = new StackPanel { Spacing = 6 };
        panel.Children.Add(Heading(heading));
        panel.Children.Add(Description(description));
        panel.Children.Add(content);
        return panel;
    }

    // A group's one-line explanation, under its heading. Shared with the account cards, which
    // draw the same heading/description/control run without Group's semibold heading, inside a
    // card that weight belongs to the account's own address.
    private static TextBlock Description(string text) =>
        new() { Text = text, TextWrapping = TextWrapping.Wrap, Opacity = 0.7 };

    // A semibold group/account heading, set via FontWeight rather than a theme Style resource,
    // which isn't reliably reachable through Application.Current.Resources from code-behind.
    private static TextBlock Heading(string text) => new() { Text = text, FontWeight = FontWeights.SemiBold };

    // --- General: language + time zone + the 12/24-hour clock ------------------

    private UIElement BuildGeneral()
    {
        var panel = new StackPanel { Spacing = 20 };

        // Language: "System", then one row per language the catalog ships, each labelled with its
        // own endonym ("Deutsch", never "German"). The rows come from L10n.Locales, so adding a
        // language to messages/ adds it here. Persisted by LanguageStore; already-loaded text only
        // re-resolves after a restart, so a change reveals the restart hint.
        var languageBox = new ComboBox { MinWidth = 220 };
        languageBox.Items.Add(new ComboBoxItem { Content = L10n.SettingsLanguageSystem(), Tag = "system" });
        foreach (var code in L10n.Locales)
        {
            languageBox.Items.Add(new ComboBoxItem { Content = L10n.LanguageName(code), Tag = code });
        }
        // "system" is row 0, so a stored locale sits at its catalog index + 1.
        var stored = Array.IndexOf(L10n.Locales, LanguageStore.Read());
        languageBox.SelectedIndex = stored < 0 ? 0 : stored + 1;

        var restart = new InfoBar
        {
            IsOpen = _languageChanged,
            IsClosable = true,
            Severity = InfoBarSeverity.Informational,
            Title = L10n.SettingsLanguageRestartTitle(),
            Message = L10n.SettingsLanguageRestartMessage(),
            Margin = new Thickness(0, 8, 0, 0),
        };
        var relaunch = new Button { Content = L10n.SettingsLanguageRestartNow() };
        relaunch.Click += (_, _) => Microsoft.Windows.AppLifecycle.AppInstance.Restart(string.Empty);
        restart.ActionButton = relaunch;

        languageBox.SelectionChanged += (_, _) =>
        {
            if (_rebuilding || (languageBox.SelectedItem as ComboBoxItem)?.Tag is not string choice)
            {
                return;
            }
            LanguageStore.Write(choice);
            LanguageStore.Apply(choice);
            _languageChanged = true;
            restart.IsOpen = true;
        };

        // Group() draws the heading as a sibling TextBlock, which is not a relation a screen
        // reader follows: every picker under one needs its own name, or it announces nothing.
        AutomationProperties.SetName(languageBox, L10n.SettingsLanguageHeading());
        var languageStack = new StackPanel { Spacing = 0 };
        languageStack.Children.Add(languageBox);
        languageStack.Children.Add(restart);
        panel.Children.Add(Group(L10n.SettingsLanguageHeading(), L10n.SettingsLanguageDescription(), languageStack));

        // Light / dark. Unlike the clock below it is drawn whether or not a core is up yet: the
        // window is already painted in an appearance, so there is a real value to show.
        panel.Children.Add(AppearanceGroup());

        // Time zone: the engine's authoritative IANA list (its bundled tzdb), not the host OS set.
        var zoneBox = new ComboBox { MinWidth = 260 };
        foreach (var zone in _model.AvailableZones)
        {
            zoneBox.Items.Add(zone);
        }
        if (!zoneBox.Items.Contains(_model.ActiveZone))
        {
            zoneBox.Items.Add(_model.ActiveZone);
        }
        zoneBox.SelectedItem = _model.ActiveZone;
        zoneBox.SelectionChanged += (_, _) =>
        {
            if (!_rebuilding && zoneBox.SelectedItem is string id)
            {
                _model.SetTimeZone(id);
            }
        };
        AutomationProperties.SetName(zoneBox, L10n.TzPickerTitle());
        panel.Children.Add(Group(L10n.TzPickerTitle(), L10n.SettingsTimezoneDescription(), zoneBox));

        // The 12/24-hour clock (SettingsDialog.Calendar.cs), under General on every platform,
        // because it spans mail AND calendar. Absent until a core exists.
        if (TimeFormatGroup() is { } timeFormat)
        {
            panel.Children.Add(timeFormat);
        }

        return panel;
    }

    // The light/dark picker. The core persists the choice; the window is repainted here, because
    // the core signals Settings alone for it, it computes nothing from the appearance, so there is
    // no snapshot for a repaint to ride in on.
    private UIElement AppearanceGroup()
    {
        var current = _model.CurrentAppearance;
        var group = "appearance";
        var stack = new StackPanel { Spacing = 4 };
        foreach (var (label, value) in new (string, Appearance)[]
        {
            (L10n.SettingsAppearanceSystem(), Appearance.System),
            (L10n.SettingsAppearanceLight(), Appearance.Light),
            (L10n.SettingsAppearanceDark(), Appearance.Dark),
        })
        {
            stack.Children.Add(Radio(label, group, current == value, () =>
            {
                _model.SetAppearance(value);
                App.Shell?.ApplyAppearance(value);
            }));
        }
        return Group(L10n.SettingsAppearanceHeading(), L10n.SettingsAppearanceDescription(), stack);
    }

    // --- Reading: conversation grouping + swipe actions ------------------------

    private UIElement BuildReading()
    {
        var panel = new StackPanel { Spacing = 20 };

        var group = "grouping";
        var stack = new StackPanel { Spacing = 4 };
        stack.Children.Add(Radio(
            L10n.SettingsGroupingThreaded(), group, _model.IsThreaded,
            () => _model.SetMode(ViewModeKind.Threaded)));
        stack.Children.Add(Radio(
            L10n.SettingsGroupingFlat(), group, !_model.IsThreaded,
            () => _model.SetMode(ViewModeKind.Flat)));
        panel.Children.Add(Group(L10n.SettingsGroupingHeading(), L10n.SettingsGroupingDescription(), stack));

        // The two per-direction swipe pickers (SettingsDialog.Swipe.cs), as on macOS and Android.
        panel.Children.Add(Group(
            L10n.SettingsSwipeHeading(), L10n.SettingsSwipeDescription(), SwipeActionControls()));

        return panel;
    }

    // --- Composing: default quote style + default send account ----------------

    private UIElement BuildComposing()
    {
        var panel = new StackPanel { Spacing = 20 };

        // The quote-style chooser (each style shown as a worked example) + the per-message opt-in.
        // It lives in SettingsDialog.QuoteStyle.cs to keep this file under the 500-line limit.
        panel.Children.Add(Group(
            L10n.QuoteStyleLabel(), L10n.SettingsComposingDescription(), QuoteStyleControl()));

        panel.Children.Add(Group(
            L10n.SettingsSendAccountHeading(), L10n.SettingsSendAccountDescription(), SendAccountControl()));

        return panel;
    }

    // Which account new mail composes from when the combined inbox is showing. Only meaningful
    // with more than one account, so with a single one the card states the sender instead of
    // offering a choice of one. The selection is read through SendAccount so a stored default
    // naming a removed account shows the first configured one, what the core would actually use.
    private UIElement SendAccountControl()
    {
        if (_model.Accounts.Count == 0)
        {
            return new TextBlock { Text = L10n.SettingsAccountsEmpty(), TextWrapping = TextWrapping.Wrap, Opacity = 0.7 };
        }
        var effective = _model.SendAccount(_model.DefaultSendAccount);
        if (_model.Accounts.Count == 1)
        {
            return new TextBlock { Text = effective?.Email ?? string.Empty, Opacity = 0.7 };
        }
        var box = new ComboBox { MinWidth = 260, DisplayMemberPath = "Email" };
        AutomationProperties.SetName(box, L10n.SettingsSendAccountHeading());
        foreach (var account in _model.Accounts)
        {
            box.Items.Add(account);
        }
        box.SelectedItem = effective;
        box.SelectionChanged += (_, _) =>
        {
            if (!_rebuilding && box.SelectedItem is AccountItem account)
            {
                _model.SetDefaultSendAccount(account.Id);
            }
        };
        return box;
    }

    // --- Advanced: AI assistant access + reset database (destructive) ---------

    private UIElement BuildAdvanced()
    {
        var panel = new StackPanel { Spacing = 20 };
        // AI assistant access (SettingsDialog.Mcp.cs) leads: it is the setting a user comes to this
        // category looking for, and putting the irreversible database reset first would make them
        // scroll past it every time. Absent until the core is up and has an endpoint.
        if (McpSection() is { } mcp)
        {
            panel.Children.Add(mcp);
        }
        panel.Children.Add(ResetDatabaseGroup());
        return panel;
    }

    private UIElement ResetDatabaseGroup()
    {
        var stack = new StackPanel { Spacing = 8 };
        var reset = new Button { Content = L10n.ActionResetDatabase() };
        // A nested ContentDialog isn't allowed, so confirm inline: the button reveals a warning
        // and a destructive "Reset now" rather than opening a second dialog.
        var confirm = new StackPanel { Spacing = 8, Visibility = Visibility.Collapsed };
        confirm.Children.Add(new TextBlock { Text = L10n.ResetMessage(), TextWrapping = TextWrapping.Wrap });
        var confirmButtons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var confirmReset = new Button { Content = L10n.ResetConfirm() };
        confirmReset.Click += (_, _) =>
        {
            _model.Reset();
            Hide();
        };
        var cancel = new Button { Content = L10n.ActionCancel() };
        cancel.Click += (_, _) => { confirm.Visibility = Visibility.Collapsed; reset.Visibility = Visibility.Visible; };
        confirmButtons.Children.Add(confirmReset);
        confirmButtons.Children.Add(cancel);
        confirm.Children.Add(confirmButtons);
        reset.Click += (_, _) => { reset.Visibility = Visibility.Collapsed; confirm.Visibility = Visibility.Visible; };
        stack.Children.Add(reset);
        stack.Children.Add(confirm);
        return Group(L10n.ActionResetDatabase(), L10n.SettingsAdvancedResetDescription(), stack);
    }

    // --- Shared helpers -------------------------------------------------------

    // A radio button whose Checked handler is attached after IsChecked is set, so the initial
    // selection doesn't fire it; only a user pick runs [onSelect]. When [rebuild] is set the pick
    // changes the panel layout (push <-> poll), so it re-renders on the next dispatcher turn.
    private RadioButton Radio(string label, string group, bool selected, Action onSelect, bool rebuild = false)
    {
        var radio = new RadioButton { Content = label, GroupName = group, IsChecked = selected };
        radio.Checked += (_, _) =>
        {
            if (_rebuilding)
            {
                return;
            }
            if (rebuild)
            {
                Apply(onSelect);
            }
            else
            {
                onSelect();
            }
        };
        return radio;
    }

    // Applies a change that affects the current panel's layout, then re-renders the selected
    // category on the next dispatcher turn so the tree isn't mutated mid-event.
    private void Apply(Action change)
    {
        if (_rebuilding)
        {
            return;
        }
        change();
        if ((_categories.SelectedItem as ListViewItem)?.Tag is string tag)
        {
            _ = DispatcherQueue.TryEnqueue(() => ShowCategory(tag));
        }
    }
}
