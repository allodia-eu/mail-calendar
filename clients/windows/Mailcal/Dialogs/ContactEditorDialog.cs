// The contact editor, one dialog for both create and edit, the Windows twin of Android's
// ContactEditorSheet and Apple's ContactEditorView. First and last name, organisation, role, a
// repeating email field, a repeating phone field, and (on a create with more than one writable
// book) where to file it.
//
// Every decision lives in the pure ContactEditing (Services/, unit-tested in Mailcal.Tests): what
// the form is refused for, which intent it becomes, and how a book or a card is labelled. This
// file is the WinUI chrome over it, built imperatively like EventEditorDialog so there is no
// client-side state to drift and the tree is deterministic.
//
// The value fields are LISTS the user adds to and removes from, and their order is the card's
// order: the first address is the person's primary one, which is what the avatar and the list row
// are keyed on.

using System.Collections.Generic;
using System.Linq;
using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Dialogs;

/// <summary>The create/edit contact form. Read <see cref="Intent"/> after a Primary result.</summary>
public sealed class ContactEditorDialog : ContentDialog
{
    private readonly EditedCard? _editing;
    private readonly IReadOnlyList<ContactBookChoice> _books;

    private readonly TextBox _givenName = Field(L10n.ContactsFirstName(), "ContactGivenName");
    private readonly TextBox _surname = Field(L10n.ContactsLastName(), "ContactSurname");
    private readonly TextBox _organization =
        Field(L10n.ContactsSectionOrganizations(), "ContactOrganization");
    private readonly TextBox _title = Field(L10n.ContactsSectionTitles(), "ContactTitle");
    private readonly StackPanel _emailRows = new() { Spacing = 6 };
    private readonly StackPanel _phoneRows = new() { Spacing = 6 };
    private readonly ComboBox _bookPicker = new() { HorizontalAlignment = HorizontalAlignment.Stretch };
    private readonly TextBlock _error = new()
    {
        TextWrapping = TextWrapping.Wrap,
        Visibility = Visibility.Collapsed,
    };

    /// <summary>The intent the form built, set when the user saved a valid one.</summary>
    internal Intent? Intent { get; private set; }

    /// <summary>Builds the editor for a create (<paramref name="editing"/> null) or an edit.</summary>
    internal ContactEditorDialog(
        EditedCard? editing,
        ContactEdit seed,
        IReadOnlyList<ContactBookChoice> books)
    {
        _editing = editing;
        _books = books;

        Title = editing is null ? L10n.ContactsNew() : L10n.ContactsEdit();
        PrimaryButtonText = L10n.ActionSave();
        CloseButtonText = L10n.ActionCancel();
        DefaultButton = ContentDialogButton.Primary;

        _givenName.Text = seed.GivenName;
        _surname.Text = seed.Surname;
        _organization.Text = seed.Organization;
        _title.Text = seed.Title;
        foreach (var value in ContactEditing.ValueRows(seed.Emails))
        {
            AddValueRow(_emailRows, EmailField(), value);
        }
        foreach (var value in ContactEditing.ValueRows(seed.Phones))
        {
            AddValueRow(_phoneRows, PhoneField(), value);
        }

        Content = new ScrollViewer { Content = BuildForm(), MinWidth = 420, MaxHeight = 520 };

        // The caret opens where the work starts: the empty first field of a new contact. On
        // Opened, not here: a dialog's content has no focus to take until it is shown, and the
        // call is dropped without complaining.
        if (editing is null)
        {
            Opened += (_, _) => _givenName.Focus(FocusState.Programmatic);
        }
        // Closing is refused while the form cannot be saved, and the reason is stated under it:
        // retrying the same form would be refused the same way, so a dialog that simply shut
        // would look like a save that silently did nothing.
        PrimaryButtonClick += (_, args) =>
        {
            Intent = ContactEditing.IntentFor(Form(), _editing, SelectedBook());
            if (Intent is not null)
            {
                return;
            }
            args.Cancel = true;
            _error.Text = ContactEditing.Validate(Form()) == ContactFormError.Empty
                ? L10n.ContactsEditorInvalid()
                : L10n.ContactsEditorInvalidEmail();
            _error.Visibility = Visibility.Visible;
        };
    }

    /// <summary>The values on screen, in the order they are drawn.</summary>
    private ContactEdit Form() => new(
        _givenName.Text,
        _surname.Text,
        _organization.Text,
        _title.Text,
        [.. Values(_emailRows)],
        [.. Values(_phoneRows)]);

    private ContactBookChoice? SelectedBook()
    {
        if (_editing is not null || _books.Count == 0)
        {
            return null;
        }
        var index = _bookPicker.SelectedIndex;
        return index >= 0 && index < _books.Count
            ? _books[index]
            : ContactEditing.DefaultBook(_books);
    }

    private StackPanel BuildForm()
    {
        var form = new StackPanel { Spacing = 10, Margin = new Thickness(0, 4, 0, 4) };
        form.Children.Add(_givenName);
        form.Children.Add(_surname);
        form.Children.Add(_organization);
        form.Children.Add(_title);
        form.Children.Add(Heading(L10n.ContactsSectionEmails()));
        form.Children.Add(_emailRows);
        form.Children.Add(AddButton(L10n.ContactsAddEmail(), _emailRows, EmailField()));
        form.Children.Add(Heading(L10n.ContactsSectionPhones()));
        form.Children.Add(_phoneRows);
        form.Children.Add(AddButton(L10n.ContactsAddPhone(), _phoneRows, PhoneField()));
        // Only a create files a contact somewhere new, and only when there is a choice to make:
        // one address book is a fact, not a decision.
        if (_editing is null && _books.Count > 1)
        {
            form.Children.Add(Heading(L10n.ContactsAddressBook()));
            _bookPicker.ItemsSource = _books.Select(book => book.Label).ToList();
            _bookPicker.SelectedIndex = _books.ToList().FindIndex(book => book.IsDefault);
            if (_bookPicker.SelectedIndex < 0)
            {
                _bookPicker.SelectedIndex = 0;
            }
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(
                _bookPicker,
                L10n.ContactsPickAddressBook());
            form.Children.Add(_bookPicker);
        }
        form.Children.Add(_error);
        return form;
    }

    private Button AddButton(string label, StackPanel rows, ValueField field)
    {
        var add = new Button { Content = label, HorizontalAlignment = HorizontalAlignment.Left };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(add, label);
        add.Click += (_, _) =>
        {
            var box = AddValueRow(rows, field, string.Empty);
            box.Focus(FocusState.Programmatic);
        };
        return add;
    }

    /// <summary>Appends one value row, returning its text box.</summary>
    private static TextBox AddValueRow(StackPanel rows, ValueField field, string value)
    {
        var box = Field(field.Label, field.ValueId, header: false);
        box.Text = value;
        var remove = new Button
        {
            Content = new FontIcon { Glyph = "", FontSize = 14 },
            VerticalAlignment = VerticalAlignment.Center,
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(remove, field.RemoveLabel);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(remove, field.RemoveId);
        ToolTipService.SetToolTip(remove, field.RemoveLabel);
        var row = new Grid { ColumnSpacing = 6 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        Grid.SetColumn(box, 0);
        Grid.SetColumn(remove, 1);
        row.Children.Add(box);
        row.Children.Add(remove);
        remove.Click += (_, _) => rows.Children.Remove(row);
        rows.Children.Add(row);
        return box;
    }

    /// <summary>
    /// One repeating value field: what its rows are called, and what a test addresses them by.
    /// </summary>
    /// <remarks>
    /// The ids repeat down the field, one pair per row, which is what makes "the rows of this
    /// field" a query. They exist because the labels are localised, so a UI assertion written
    /// against a name asserts the app's language as much as its layout.
    /// </remarks>
    private sealed record ValueField(
        string Label,
        string RemoveLabel,
        string ValueId,
        string RemoveId);

    private static ValueField EmailField() => new(
        L10n.ContactsSectionEmails(),
        L10n.ContactsRemoveEmail(),
        "ContactEmailValue",
        "ContactEmailRemove");

    private static ValueField PhoneField() => new(
        L10n.ContactsSectionPhones(),
        L10n.ContactsRemovePhone(),
        "ContactPhoneValue",
        "ContactPhoneRemove");

    /// <summary>The values in one repeating field, in the order they are drawn.</summary>
    private static IEnumerable<string> Values(StackPanel rows) =>
        rows.Children.OfType<Grid>().Select(row => row.Children.OfType<TextBox>().First().Text);

    /// <summary>
    /// A text box labelled <paramref name="label"/>, headed by it unless the field already
    /// carries a heading of its own.
    /// </summary>
    /// <remarks>
    /// A repeating field's rows take <c>header: false</c>. The section heading above them names
    /// the field once, so a header per row repeats it, and it also makes the row taller than the
    /// box: the remove button centres on the whole cell and ends up level with the label rather
    /// than with the value it removes. The accessible name stays either way, so a row is still
    /// announced as what it holds.
    /// </remarks>
    private static TextBox Field(string label, string? automationId, bool header = true)
    {
        var box = new TextBox { PlaceholderText = label };
        if (header)
        {
            box.Header = label;
        }
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, label);
        if (automationId is not null)
        {
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(box, automationId);
        }
        return box;
    }

    private static TextBlock Heading(string text) => new()
    {
        Text = text,
        Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
    };
}

/// <summary>
/// Asks which account's card to edit, when the person is filed in more than one.
/// </summary>
/// <remarks>
/// Its own step rather than a picker inside the editor, because the answer decides what the form
/// is <em>seeded with</em>: a merged person's values belong to different cards, and letting the
/// user change accounts mid-edit would have to throw away what they had typed.
/// </remarks>
public sealed class ContactCardChoiceDialog : ContentDialog
{
    private readonly IReadOnlyList<ContactCardChoice> _cards;
    private readonly ListView _list = new();

    /// <summary>The card the user picked, set after a Primary result.</summary>
    internal ContactCardChoice? Picked =>
        _list.SelectedIndex >= 0 && _list.SelectedIndex < _cards.Count
            ? _cards[_list.SelectedIndex]
            : null;

    internal ContactCardChoiceDialog(IReadOnlyList<ContactCardChoice> cards)
    {
        _cards = cards;
        Title = L10n.ContactsEdit();
        PrimaryButtonText = L10n.ActionEdit();
        CloseButtonText = L10n.ActionCancel();
        DefaultButton = ContentDialogButton.Primary;
        _list.ItemsSource = cards.Select(card => card.Label).ToList();
        _list.SelectedIndex = 0;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAutomationId(_list, "ContactCardChoice");
        Content = new StackPanel
        {
            Spacing = 10,
            Children =
            {
                new TextBlock { Text = L10n.ContactsPickCard(), TextWrapping = TextWrapping.Wrap },
                _list,
            },
        };
    }
}
