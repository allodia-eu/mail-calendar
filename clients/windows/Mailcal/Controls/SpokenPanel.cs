// A picture that is read as one sentence: a panel, which has no automation peer of its own, given
// one so its AutomationProperties.Name reaches assistive technology, with everything drawn inside it
// left out of the tree.

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Controls;

internal sealed partial class SpokenPanel : Grid
{
    /// <summary>Draws <paramref name="child"/> and speaks <paramref name="spoken"/> in its place; an
    /// empty sentence leaves the whole picture out of the tree.</summary>
    internal SpokenPanel(UIElement child, string spoken)
    {
        Children.Add(child);
        Hide(child);
        if (spoken.Length > 0)
        {
            AutomationProperties.SetName(this, spoken);
        }
        else
        {
            AutomationProperties.SetAccessibilityView(this, AccessibilityView.Raw);
        }
    }

    private static void Hide(UIElement element)
    {
        AutomationProperties.SetAccessibilityView(element, AccessibilityView.Raw);
        switch (element)
        {
            case Panel panel:
                foreach (var child in panel.Children)
                {
                    Hide(child);
                }
                break;
            case Border { Child: { } child }:
                Hide(child);
                break;
        }
    }

    protected override AutomationPeer OnCreateAutomationPeer() => new SpokenPeer(this);

    private sealed partial class SpokenPeer(SpokenPanel owner) : FrameworkElementAutomationPeer(owner)
    {
        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.Text;

        protected override string GetClassNameCore() => nameof(SpokenPanel);
    }
}
