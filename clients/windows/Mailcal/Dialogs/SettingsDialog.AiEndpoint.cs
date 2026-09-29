// Settings → Advanced → Own AI endpoint (docs/ai.md, "Where requests go"; docs/settings.md slot 11).
// Always drawn: it is how a build without Allodia's relay gets writing style at all, and saving
// one is what puts the Writing style category in the list, so the list is brought up to date here
// rather than on the next opening.
//
// The key field is never filled back in. An empty one keeps the stored key (OwnEndpointForm), and
// the line under it says whether there is one.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // Why the last removal could not delete the key. It survives the redraw that removal causes,
    // since the settings are gone either way and the form below has to show that.
    private OwnEndpointProblem? _endpointProblem;

    private UIElement OwnAiEndpointGroup()
    {
        var current = _model.OwnEndpoint;
        var stack = new StackPanel { Spacing = 8 };

        var address = new TextBox { Header = L10n.AiEndpointAddress(), Text = current?.BaseUrl ?? string.Empty };
        AutomationProperties.SetAutomationId(address, "AiEndpointAddress");
        stack.Children.Add(address);
        stack.Children.Add(Description(L10n.AiEndpointAddressHint()));

        var key = new PasswordBox { Header = L10n.AiEndpointKey() };
        AutomationProperties.SetAutomationId(key, "AiEndpointKey");
        stack.Children.Add(key);
        stack.Children.Add(Description(
            current?.HasKey == true ? L10n.AiEndpointKeyStored() : L10n.AiEndpointKeyHint()));

        var model = new TextBox { Header = L10n.AiEndpointModel(), Text = current?.Model ?? string.Empty };
        AutomationProperties.SetAutomationId(model, "AiEndpointModel");
        stack.Children.Add(model);

        // Unknown until the person says, which the gate refuses under every EU mode.
        var declared = current?.Declared;
        var where = new StackPanel { Spacing = 4 };
        where.Children.Add(new TextBlock { Text = L10n.AiEndpointWhere(), TextWrapping = TextWrapping.Wrap });
        foreach (var (label, value) in new (string, JurisdictionClass)[]
        {
            (L10n.AiEndpointWhereEuNative(), JurisdictionClass.EuNative),
            (L10n.AiEndpointWhereEuHosted(), JurisdictionClass.EuHosted),
            (L10n.AiEndpointWhereNonEu(), JurisdictionClass.NonEu),
        })
        {
            where.Children.Add(Radio(label, "ai-endpoint-where", declared == value, () => declared = value));
        }
        stack.Children.Add(where);

        var error = Warning(string.Empty);
        error.Visibility = Visibility.Collapsed;
        if (_endpointProblem is { } problem)
        {
            error.Text = WritingStyleText.Of(problem);
            error.Visibility = Visibility.Visible;
            _endpointProblem = null;
        }

        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var save = new Button { Content = L10n.AiEndpointSave() };
        AutomationProperties.SetAutomationId(save, "AiEndpointSave");
        save.Click += (_, _) =>
        {
            var refused = _model.SaveOwnEndpoint(
                address.Text, model.Text, declared, OwnEndpointForm.KeyArgument(key.Password));
            if (refused is { } why)
            {
                // Said in place, so the fields keep what was typed.
                error.Text = WritingStyleText.Of(why);
                error.Visibility = Visibility.Visible;
                return;
            }
            SyncCategories();
            Apply(() => { });
        };
        buttons.Children.Add(save);
        if (current is not null)
        {
            var remove = new Button { Content = L10n.AiEndpointRemove() };
            remove.Click += (_, _) =>
            {
                _endpointProblem = _model.RemoveOwnEndpoint();
                SyncCategories();
                Apply(() => { });
            };
            buttons.Children.Add(remove);
        }
        stack.Children.Add(buttons);
        stack.Children.Add(error);
        return Group(L10n.AiEndpointTitle(), L10n.AiEndpointIntro(), stack);
    }
}
