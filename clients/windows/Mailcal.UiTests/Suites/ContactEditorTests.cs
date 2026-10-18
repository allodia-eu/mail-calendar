// The contacts editor, driven up to the point of a write and no further (docs/contacts.md §3).
//
// WHY HERE. What the editor decides lives in ContactEditing, which Mailcal.Tests gates. What it
// cannot see is anything WinUI-shaped, and this suite was written against two such failures:
//
//   * The create affordance is hidden unless the core reports a writable address book, so its
//     presence is the JMAP write-destination binding working. A broken binding would read as a
//     product decision ("this server refuses writes").
//   * Each repeating value row carried its own TextBox.Header, so the field's name was drawn twice
//     and the row's remove button, centred on the whole cell, sat level with the label instead of
//     with the box it removes.
//
// WHY IT STOPS BEFORE THE WRITE. The app offers no delete, so a create would leave a card behind in
// the shared harness on every run. The write is gated in mailcal-app's contacts suite against a
// real engine. So the status line a settled write draws is not gated here: it is collapsed, and out
// of the tree, until a write settles (docs/contacts.md §3).
//
// WHY THE HARNESS. The create affordance is a real capability read off a real server.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
public sealed class ContactEditorTests(HarnessApp harness)
{
    private void Case(string name, Action<App> body) => harness.Case(name, body, app => app.CloseDialogs());

    /// <summary>The contacts surface on screen; the launch is on mail.</summary>
    private static void ShowContacts(App app)
    {
        if (app.Agent.Find(Filter.ById("ContactsList")) is not null)
        {
            return;
        }
        app.Agent.Invoke(app.Agent.Find(Filter.ById("NavContacts")) ?? throw new InvalidOperationException("no Contacts entry in the navigation pane"));
        if (app.Agent.Wait(Filter.ById("ContactsList"), TimeSpan.FromSeconds(60)) is null)
        {
            throw new InvalidOperationException("the contacts list did not come up");
        }
    }

    /// <summary>The create button. Its absence is the interesting failure: it is hidden unless the
    /// core reports a writable address book.</summary>
    private static Element NewContact(App app)
    {
        ShowContacts(app);
        return app.Agent.Wait(Filter.ById("ContactsNew", "Button"), TimeSpan.FromSeconds(60))
            ?? throw new InvalidOperationException(
                "no New contact button (#ContactsNew): either the harness account lost its contacts destination or the JMAP adapter was left unbound (docs/contacts.md §3)");
    }

    private static Element OpenCreateForm(App app)
    {
        app.Agent.Invoke(NewContact(app));
        return app.Dialog(Filter.ById("ContactGivenName", "Edit"));
    }

    private static Element InForm(App app, Element dialog, string id, string type) =>
        app.Agent.Find(new Filter { Id = id, Type = type, Under = dialog }) ?? throw new InvalidOperationException($"the contact editor has no #{id}");

    [HarnessFact]
    public void TheCreateAffordanceIsOfferedAtAll() => Case(nameof(TheCreateAffordanceIsOfferedAtAll), app =>
    {
        Assert.True(NewContact(app).Enabled, "the New contact button is on screen but disabled, so the core offered a destination the form cannot reach");
    });

    [HarnessFact]
    public void AnEmptyFormIsRefusedAndSaysWhyRatherThanClosing() => Case(nameof(AnEmptyFormIsRefusedAndSaysWhyRatherThanClosing), app =>
    {
        var dialog = OpenCreateForm(app);
        var before = app.Agent.FindAll(new Filter { Type = "Text", Under = dialog }).Count;
        app.Agent.Invoke(InForm(app, dialog, "PrimaryButton", "Button"));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
        Assert.True(app.Agent.Find(Filter.ById("ContactGivenName", "Edit")) is not null,
            "Save closed the editor on an empty form: the core refuses a card with nothing to file it under, so nothing is written and nobody is told (docs/contacts.md §3)");
        // Counted rather than matched, because the sentence is localised: the refusal put one on
        // screen. A dialog that just sits there reads as a dead button.
        var after = app.Agent.FindAll(new Filter { Type = "Text", Under = app.Dialog(Filter.ById("ContactGivenName", "Edit")) }).Count;
        Assert.True(after > before, "the editor refused the form without stating a reason under it");
    });

    [HarnessFact]
    public void ARepeatingRowIsABareValueBoxWithItsRemoveButtonInLineWithIt() => Case(nameof(ARepeatingRowIsABareValueBoxWithItsRemoveButtonInLineWithIt), app =>
    {
        var dialog = OpenCreateForm(app);
        var boxes = app.Agent.FindAll(new Filter { Id = "ContactEmailValue", Type = "Edit", Under = dialog });
        Assert.True(boxes.Count == 1, "the email field did not open on exactly one empty row, so the geometry below would measure something else");
        var box = app.Steady(boxes[0]);
        // Against a field that does carry a header, rather than a constant: the two are the same
        // control, so the difference in height is the header, whatever the scale, theme or font.
        var headed = InForm(app, dialog, "ContactGivenName", "Edit");
        Assert.True(box.Height < headed.Height,
            $"a repeating row's value box is {box.Height}px tall, as tall as the headed First name field: it carries a header of its own, drawing the field's name twice");
        // The button pinned to the row's top or bottom. This cannot catch the header: a text box's
        // rectangle includes its header, so a headed box and its button share a centre.
        var remove = InForm(app, dialog, "ContactEmailRemove", "Button");
        var drift = Math.Abs((box.Y + (box.Height / 2)) - (remove.Y + (remove.Height / 2)));
        Assert.True(drift <= 4 * Input.Scale(box.Window),
            $"the remove button's centre is {drift}px off the value box's, so it is not centred on the value it removes");
    });

    [HarnessFact]
    public void AValueRowIsLabelledOnceNotTwice() => Case(nameof(AValueRowIsLabelledOnceNotTwice), app =>
    {
        // The section heading names the field; a header on each row repeats it down the form. Read
        // off the box's own subtree, so the heading, a sibling, is not counted, and against the headed
        // First name field: a box's placeholder is a text of its own in either, a header the extra one.
        var dialog = OpenCreateForm(app);
        string[] TextsIn(string id) =>
            app.Agent.FindAll(new Filter { Type = "Text", Under = InForm(app, dialog, id, "Edit"), OnScreen = true }).Select(t => t.Name).ToArray();
        var row = TextsIn("ContactEmailValue");
        var headed = TextsIn("ContactGivenName");
        Assert.True(row.Length < headed.Length,
            $"the email value box draws {row.Length} text(s) of its own ({string.Join(", ", row)}), as many as the headed First name field ({string.Join(", ", headed)}); the heading above already names the field");
    });
}
