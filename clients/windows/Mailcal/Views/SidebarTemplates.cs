// The WinUI half of the data-bound sidebar: which DataTemplate a node gets, and the one type
// conversion the templates need. Kept beside the views rather than in ViewModels/ because both
// touch WinUI types, and SidebarItem/SidebarTree must stay free of them (they are linked into
// Mailcal.Tests, a plain net10.0 assembly).

using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Data;

namespace Allodia.Mailcal.Views;

/// <summary>
/// Picks the account template (which carries the unreachable badge, the folder children and the
/// "Remove account" context menu) for an account node, and the plain one for everything else,
/// All Inboxes, each folder, and Add account.
/// </summary>
/// <remarks>
/// <para>
/// A folder must NOT get the account template: its context menu would offer to remove an account,
/// which is destructive and names the wrong thing. The discriminator is
/// <see cref="SidebarItem.AccountId"/>, which is non-null on exactly the account rows.
/// </para>
/// <para>
/// One selector serves BOTH lists: a NavigationView applies its menu-item template to its footer
/// items as well. That is why the two footer destinations are data now (MainWindow.Sidebar.cs's
/// <c>FooterItems</c>) rather than the XAML-declared NavigationViewItems they used to be, left as
/// XAML they were re-templated anyway, and came out looking perfectly right: same label, still
/// navigable, because the plain template binds <c>Content</c> and <c>Tag</c>, which a
/// NavigationViewItem also has. What they silently lost was their
/// <c>AutomationProperties.AutomationId</c>, so every automation lookup of NavCalendar/NavContacts
/// broke while the app on screen looked untouched. The uitests caught it; a screenshot could not.
/// Hence <see cref="SidebarItem.AutomationId"/>, and hence no <c>null</c> branch below, returning
/// null to mean "leave this one alone" is not something a DataTemplateSelector may do (WinUI
/// rejects it outright: "Null encountered as data template").
/// </para>
/// </remarks>
public sealed partial class SidebarItemTemplateSelector : DataTemplateSelector
{
    /// <summary>The template for an account row.</summary>
    public DataTemplate? Account { get; set; }

    /// <summary>The template for every other <see cref="SidebarItem"/>.</summary>
    public DataTemplate? Plain { get; set; }

    /// <inheritdoc/>
    protected override DataTemplate? SelectTemplateCore(object item) =>
        item is SidebarItem { AccountId: not null } ? Account : Plain;

    /// <inheritdoc/>
    protected override DataTemplate? SelectTemplateCore(object item, DependencyObject container) =>
        SelectTemplateCore(item);
}

/// <summary>
/// A bound <c>bool</c> as a <see cref="Visibility"/>, for the account row's unreachable badge.
/// </summary>
/// <remarks>
/// The rest of the app needs no converter at all, its row types expose <c>Visibility</c> directly
/// (ViewModels/RowViewModels.cs) so the XAML stays declarative. <see cref="SidebarItem"/> cannot:
/// it is linked into Mailcal.Tests, a plain net10.0 assembly, so a WinUI type on it would stop that
/// project compiling. Hence one small converter here, on the WinUI side of the line.
/// </remarks>
public sealed partial class BoolToVisibilityConverter : IValueConverter
{
    /// <inheritdoc/>
    public object Convert(object value, Type targetType, object parameter, string language) =>
        value is true ? Visibility.Visible : Visibility.Collapsed;

    /// <inheritdoc/>
    public object ConvertBack(object value, Type targetType, object parameter, string language) =>
        value is Visibility.Visible;
}
