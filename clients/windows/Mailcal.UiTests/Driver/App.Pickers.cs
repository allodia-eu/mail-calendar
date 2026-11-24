// A picker's open list: the items a person sees once a ComboBox is open, and presses.

namespace Allodia.Mailcal.UiTests.Driver;

internal sealed partial class App
{
    /// <summary>
    /// Opens <paramref name="picker"/> and answers the items of its open list, in its order.
    /// </summary>
    /// <remarks>
    /// The open list is a popup whose parent is the picker, and its items are the ones drawn. The
    /// picker also lists its items under itself, by the same names and in the same place, and those
    /// take no press. They cannot be told apart by handle: UI Automation makes the picker's peers
    /// anew between two lookups, so the picker and its own items come back under new handles. Only
    /// where an item sits in the tree tells them apart. A list still opening has drawn only some of
    /// its items, so it is read until two reads agree.
    /// </remarks>
    internal List<Element> OpenList(Element picker)
    {
        Agent.Expand(picker);
        List<Element> items = [];
        var previous = "";
        if (!Eventually(() =>
            {
                items = Drawn(picker);
                var read = string.Join("\n", items.Select(item => $"{item.Name} {item.X},{item.Y}"));
                var agreed = items.Count > 0 && read == previous;
                previous = read;
                return agreed;
            }, TimeSpan.FromSeconds(5)))
        {
            throw new InvalidOperationException($"'{picker.Name}' (#{picker.Id}) opened no list");
        }
        return items;
    }

    /// <summary>The items <paramref name="picker"/>'s open list draws now; none while it is closed.
    /// For a caller that opens the picker itself (see <see cref="OpenList"/>).</summary>
    /// <remarks>
    /// The agent walks a popup from the popup, so a lookup under the picker does not reach its open
    /// list: the list is found among the popups, as the one whose parent is the picker.
    /// </remarks>
    internal List<Element> Drawn(Element picker) =>
        Agent.FindAll(new Filter { Type = "Window", Window = picker.Window })
            .Where(popup => Agent.Parent(popup) is { } parent && Same(parent, picker))
            .SelectMany(popup => Agent.FindAll(new Filter { Type = "ListItem", Under = popup }))
            .Where(item => item.Rendered)
            .ToList();

    // The same control read twice, under two handles. Not by its rectangle: opening a picker
    // scrolls it into view.
    private static bool Same(Element a, Element b) =>
        a.Type == b.Type && a.Id == b.Id && a.Name == b.Name && a.Class == b.Class && a.Window == b.Window;
}
