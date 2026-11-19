// The Settings dialog's categories: which exist, in which order, and the row each is drawn as
// (docs/settings.md).

using System.Linq;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The category rows, in display order, tag (used to dispatch) + localised label.
    //
    // Allodia is first and is dropped whole in a build carrying no registration: a row that opens
    // an empty panel reads as a broken panel rather than as a build without a route, and it is what
    // every build from source would show. The category and the group inside it share one string,
    // because the product's own name belongs in one place.
    private (string Tag, string Label)[] Categories() =>
        MailcalBindingsMethods.AllodiaSignInAvailable()
            ? new[] { ("allodia", L10n.SettingsAllodiaHeading()) }.Concat(MailCategories()).ToArray()
            : MailCategories();

    private (string Tag, string Label)[] MailCategories() => new (string, string)?[]
    {
        ("general", L10n.SettingsCategoryGeneral()),
        ("calendar", L10n.SettingsCategoryCalendar()),
        ("videoconferencing", L10n.SettingsCategoryVideoconferencing()),
        ("reading", L10n.SettingsCategoryReading()),
        ("composing", L10n.SettingsCategoryComposing()),
        // Its own category, not a sub-screen of Composing: a signature is a standalone entity reused
        // across accounts, and "Settings → Signatures" is the path people already look for
        // (docs/settings.md, Signatures).
        ("signatures", L10n.SettingsCategorySignatures()),
        // Only while the writing assistant is offered: the core's page is null otherwise, and so
        // the category is absent (docs/ai.md rule 4).
        _model.WritingStylePage() is null ? null : (WritingStyleTag, L10n.SettingsCategoryWritingStyle()),
        ("notifications", L10n.SettingsCategoryNotifications()),
        ("privacy", L10n.SettingsCategoryPrivacy()),
        ("accounts", L10n.SettingsCategoryAccounts()),
        ("advanced", L10n.SettingsCategoryAdvanced()),
        ("diagnostics", L10n.SettingsCategoryDiagnostics()),
        ("about", L10n.SettingsCategoryAbout()),
    }.OfType<(string, string)>().ToArray();

    // The row's id is its tag, so a test finds a category in whatever language the app is in.
    private static ListViewItem CategoryItem(string tag, string label)
    {
        var item = new ListViewItem { Content = label, Tag = tag };
        AutomationProperties.SetAutomationId(item, $"SettingsCategory_{tag}");
        return item;
    }
}
