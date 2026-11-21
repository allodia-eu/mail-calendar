// A line with a state that nothing presses: an open or a done circle beside the text, read as one
// element whose state is its toggle state, with no action behind it. It stands where a checkbox held
// would read as unavailable rather than as following something else (docs/ai.md rule 23).

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Automation.Provider;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Allodia.Mailcal.Controls;

internal sealed partial class StateMark : StackPanel
{
    private const string OpenGlyph = "";
    private const string DoneGlyph = "";

    private readonly bool _done;

    internal StateMark(string text, bool done, string automationId)
    {
        _done = done;
        Orientation = Orientation.Horizontal;
        Spacing = 10;
        // A checkbox's height, so a mark and a tick in one list line up.
        MinHeight = 32;
        var mark = new FontIcon
        {
            Glyph = done ? DoneGlyph : OpenGlyph,
            FontSize = 18,
            Width = 20,
            VerticalAlignment = VerticalAlignment.Center,
            Foreground = (Brush)Application.Current.Resources[done ? "AccentTextFillColorPrimaryBrush" : "TextFillColorSecondaryBrush"],
        };
        var line = new TextBlock { Text = text, TextWrapping = TextWrapping.Wrap, VerticalAlignment = VerticalAlignment.Center };
        AutomationProperties.SetAccessibilityView(mark, AccessibilityView.Raw);
        AutomationProperties.SetAccessibilityView(line, AccessibilityView.Raw);
        Children.Add(mark);
        Children.Add(line);
        AutomationProperties.SetName(this, text);
        AutomationProperties.SetAutomationId(this, automationId);
    }

    protected override AutomationPeer OnCreateAutomationPeer() => new MarkPeer(this);

    // Its state is read through the toggle pattern, which is where assistive technology reads a
    // done or open item; toggling it is refused, since what it follows decides it.
    private sealed partial class MarkPeer(StateMark owner) : FrameworkElementAutomationPeer(owner), IToggleProvider
    {
        public ToggleState ToggleState => owner._done ? ToggleState.On : ToggleState.Off;

        public void Toggle() => throw new InvalidOperationException("this state follows what it marks and is not pressed");

        protected override object? GetPatternCore(PatternInterface patternInterface) =>
            patternInterface == PatternInterface.Toggle ? this : base.GetPatternCore(patternInterface);

        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.Text;

        protected override string GetClassNameCore() => nameof(StateMark);
    }
}
