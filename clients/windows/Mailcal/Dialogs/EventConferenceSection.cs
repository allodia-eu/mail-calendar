// An event's video meetings in its detail (docs/conferencing.md): Join first, then what a person
// needs to get in another way, the meeting ID, the passcode and the numbers to dial. The Windows
// twin of Apple's EventConferenceView; the decisions are Calendar/EventMeeting.cs.
//
// Every string here came from whoever wrote the event, so each goes in as `TextBlock.Text`, and the
// links open through the OS on a press, never in a view of the app's own. Nothing here logs: a Zoom
// link carries its passcode.
using System;
using System.Runtime.InteropServices;
using Allodia.Mailcal.Calendar;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Windows.ApplicationModel.DataTransfer;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>Builds one meeting's block of the event detail.</summary>
internal static class EventConferenceSection
{
    /// <summary>
    /// One meeting: its heading with Join, then each row it has a value for. <paramref name="index"/>
    /// is its place in the core's order, which keeps each meeting's AutomationIds its own: the first
    /// carries the bare ids, a later one the same ids suffixed with its index.
    /// </summary>
    internal static StackPanel Build(EventConference conference, int index, ThemeBrushes brushes)
    {
        string Id(string name) => index == 0 ? name : $"{name}-{index}";
        var section = new StackPanel { Spacing = 6, Margin = new Thickness(0, 10, 0, 0) };
        var join = EventMeeting.JoinUri(conference);

        section.Children.Add(Heading(conference, join, Id));
        if (EventMeeting.Status(conference, L10n.EventMeetingPreparing(), L10n.EventMeetingFailed()) is { } status)
        {
            section.Children.Add(Secondary(status, brushes));
        }
        // The link Join opens, so a meeting with nothing to join offers nothing to copy either.
        // The link as the core sent it, not .NET's re-escaped rebuild of it.
        if (join is not null)
        {
            section.Children.Add(CopyLink(conference.JoinUrl!, Id("EventMeetingCopyLink")));
        }
        if (EventMeeting.Shows(conference.MeetingId))
        {
            section.Children.Add(Labeled(L10n.EventMeetingId(), conference.MeetingId!, Id("EventMeetingId")));
        }
        if (EventMeeting.Shows(conference.Passcode))
        {
            section.Children.Add(Labeled(L10n.EventMeetingPasscode(), conference.Passcode!, Id("EventMeetingPasscode")));
        }
        if (conference.DialIn.Length > 0)
        {
            var numbers = new StackPanel { Spacing = 2 };
            numbers.Children.Add(Caption(L10n.EventMeetingDialIn()));
            // In the core's order, one row each: a server can list one number twice with a
            // different PIN.
            foreach (var number in conference.DialIn)
            {
                numbers.Children.Add(DialInRow(number, brushes));
            }
            section.Children.Add(numbers);
        }
        if (EventMeeting.Shows(conference.Notes))
        {
            var notes = Secondary(conference.Notes!, brushes);
            notes.IsTextSelectionEnabled = true;
            section.Children.Add(notes);
        }
        return section;
    }

    // The camera and the service's name, with Join at the far end when there is a link to open.
    private static Grid Heading(EventConference conference, Uri? join, Func<string, string> id)
    {
        var heading = new Grid { ColumnSpacing = 10 };
        heading.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        heading.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        heading.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        heading.Children.Add(new FontIcon
        {
            Glyph = EventMeeting.CameraGlyph,
            FontSize = 16,
            VerticalAlignment = VerticalAlignment.Center,
        });
        var title = new TextBlock
        {
            Text = EventMeeting.Title(conference, L10n.EventVideoMeeting()),
            Style = (Style)Application.Current.Resources["BodyStrongTextBlockStyle"],
            TextWrapping = TextWrapping.Wrap,
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetAutomationId(title, id("EventMeetingService"));
        Grid.SetColumn(title, 1);
        heading.Children.Add(title);

        if (join is not null)
        {
            var button = new Button
            {
                Content = L10n.EventJoin(),
                Style = (Style)Application.Current.Resources["AccentButtonStyle"],
                VerticalAlignment = VerticalAlignment.Center,
            };
            AutomationProperties.SetAutomationId(button, id("EventJoin"));
            button.Click += (_, _) => _ = Windows.System.Launcher.LaunchUriAsync(join);
            Grid.SetColumn(button, 2);
            heading.Children.Add(button);
        }
        return heading;
    }

    private static Button CopyLink(string link, string automationId)
    {
        var button = new Button
        {
            Content = WithGlyph(EventMeeting.CopyGlyph, L10n.ActionCopyLink()),
            Style = (Style)Application.Current.Resources["SubtleButtonStyle"],
            Padding = new Thickness(6, 4, 6, 4),
        };
        AutomationProperties.SetName(button, L10n.ActionCopyLink());
        AutomationProperties.SetAutomationId(button, automationId);
        button.Click += (_, _) =>
        {
            var package = new DataPackage();
            package.SetText(link);
            // Another process holding the clipboard open makes this throw; the press then copies
            // nothing, which the person sees on paste, rather than taking the app down.
            try
            {
                Clipboard.SetContent(package);
            }
            catch (COMException)
            {
            }
        };
        return button;
    }

    // A number opens the phone app only when the core gave it a URI to dial; a number the link
    // policy refused is text.
    private static StackPanel DialInRow(ConferenceDialIn number, ThemeBrushes brushes)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        if (EventMeeting.DialInUri(number) is { } dial)
        {
            var button = new HyperlinkButton
            {
                Content = WithGlyph(EventMeeting.PhoneGlyph, number.Number),
                Padding = new Thickness(0, 2, 0, 2),
            };
            AutomationProperties.SetName(button, number.Number);
            button.Click += (_, _) => _ = Windows.System.Launcher.LaunchUriAsync(dial);
            row.Children.Add(button);
        }
        else
        {
            row.Children.Add(new TextBlock
            {
                Text = number.Number,
                IsTextSelectionEnabled = true,
                VerticalAlignment = VerticalAlignment.Center,
            });
        }
        if (EventMeeting.Shows(number.Pin))
        {
            var pin = Secondary(L10n.EventMeetingPin(number.Pin!), brushes);
            pin.IsTextSelectionEnabled = true;
            pin.VerticalAlignment = VerticalAlignment.Center;
            row.Children.Add(pin);
        }
        return row;
    }

    // `automationId` names the VALUE, as the detail's own rows do: the caption is localised.
    private static StackPanel Labeled(string label, string value, string automationId)
    {
        var text = new TextBlock { Text = value, TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true };
        AutomationProperties.SetAutomationId(text, automationId);
        return new StackPanel { Spacing = 1, Children = { Caption(label), text } };
    }

    private static StackPanel WithGlyph(string glyph, string text) => new()
    {
        Orientation = Orientation.Horizontal,
        Spacing = 6,
        Children =
        {
            new FontIcon { Glyph = glyph, FontSize = 14, VerticalAlignment = VerticalAlignment.Center },
            new TextBlock { Text = text, VerticalAlignment = VerticalAlignment.Center },
        },
    };

    private static TextBlock Caption(string text) => new()
    {
        Text = text,
        Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
    };

    // The detail's own secondary text, wrapping: a meeting's notes and status can run long.
    private static TextBlock Secondary(string text, ThemeBrushes brushes)
    {
        var block = EventDetailDialog.SecondaryOf(text, brushes);
        block.TextWrapping = TextWrapping.Wrap;
        return block;
    }
}
