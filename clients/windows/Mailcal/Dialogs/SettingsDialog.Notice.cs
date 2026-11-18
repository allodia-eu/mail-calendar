// The notice at the top of Settings (docs/notices.md rule 2): the first one for every screen,
// above whichever category is open, drawn by the window's own NoticeBar, because this dialog's
// overlay hides the window's. Which notice is NoticeRules.InSettings.

using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    private readonly NoticeBar _settingsNotice = new();

    /// <summary>The dialog's content: the notice above <paramref name="categories"/>, the category
    /// list beside its detail, which gives up the bar's height while one is drawn.</summary>
    private Grid WithSettingsNotice(Grid categories)
    {
        // A request for an account's page opens it here: the window's way opens a second dialog,
        // which WinUI does not show over this one.
        _settingsNotice.InitInSettings(_model, () => _accountPage, OpenAccountFromOtherCategory);
        _model.NoticesChanged += _settingsNotice.Draw;
        Closed += (_, _) => _model.NoticesChanged -= _settingsNotice.Draw;
        var root = new Grid { Width = 680, Height = 500 };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        Grid.SetRow(categories, 1);
        root.Children.Add(_settingsNotice);
        root.Children.Add(categories);
        return root;
    }
}
