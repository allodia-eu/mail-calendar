// The signature body editor (Settings ▸ Signatures ▸ Edit): the shared clients/composer/dist/editor.html
// bundle hosted body-only, through the SAME EditorWebViewHost the composer uses, one definition of
// the gates, so the two cannot drift (docs/composer-security.md, docs/signatures.md). Authoring a
// signature is authoring mail content, so it gets the composer's gates, not a lighter set.
//
// It renders INSIDE the settings detail panel rather than in a dialog of its own, because a nested
// ContentDialog is not allowed, the same constraint that made the destructive database reset
// confirm in place.
//
// What it may save is the core's: Save is the page's, held besides while the core refuses the name
// typed, a picture is asked about before it is read, and what a save came to is the page's outcome
// (SignatureEditorSession). It inserts a picture as a self-contained `data:` URI, which is what a
// signature stores and what the core rewrites to a `cid:` part on send.

using Allodia.Mailcal.Controls;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.Views;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Windows.Storage.Pickers;
using uniffi.mailcal_bindings;
using Windows.Storage;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The live editor host, held so the WebView2 can be released when the panel is rebuilt or the
    // dialog closes. A WebView2 dropped from the tree is not disposed by dropping it.
    private EditorWebViewHost? _signatureEditor;

    // The open editor's parts a read of the page draws again, or null while none is open.
    private SignatureEditorParts? _signatureParts;

    /// <summary>Releases the signature editor's WebView2, if one is up. Called before every detail
    /// rebuild and when the dialog closes.</summary>
    private void CloseSignatureEditor()
    {
        _signatureEditor?.Close();
        _signatureEditor = null;
        _signatureParts = null;
    }

    private UIElement BuildSignatureEditor(EditingSignature editing, SignaturesPage page)
    {
        var panel = new StackPanel { Spacing = 8 };
        panel.Children.Add(Heading(editing.IsNew ? L10n.SettingsSignaturesAdd() : editing.Name));

        var nameBox = new TextBox
        {
            Header = L10n.SettingsSignaturesNameLabel(),
            PlaceholderText = L10n.SettingsSignaturesNamePlaceholder(),
            Text = editing.Name,
            MinWidth = 240,
            HorizontalAlignment = HorizontalAlignment.Left,
        };
        HistoryKeys.RedoOnShiftChord(nameBox);
        panel.Children.Add(nameBox);

        panel.Children.Add(new TextBlock
        {
            Text = L10n.SettingsSignaturesBodyLabel(),
            Margin = new Thickness(0, 8, 0, 0),
        });

        // An explicit height: the detail panel lives in a ScrollViewer, which offers infinite height
        // during measure, and a WebView2 given that lays out at zero.
        // Transparent until the editor paints, so the panel shows rather than WebView2's own white.
        var view = new WebView2 { DefaultBackgroundColor = Microsoft.UI.Colors.Transparent };
        panel.Children.Add(new Border
        {
            BorderBrush = _brushes.Of(ThemePalette.ControlStroke),
            BorderThickness = new Thickness(1),
            Height = 230,
            Child = view,
        });

        // What was refused, a picture or a save.
        var said = new TextBlock
        {
            Visibility = Visibility.Collapsed,
            TextWrapping = TextWrapping.Wrap,
            Foreground = _brushes.Of(ThemePalette.Critical),
            Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
        };

        // The signature is written on the page a reading view would draw it on
        // (docs/reading-dark-mode.md, "Writing in a dark app").
        var host = new EditorWebViewHost(view) { Model = _model };
        _signatureEditor = host;
        var parts = new SignatureEditorParts(editing, host, nameBox, said, page);
        _signatureParts = parts;
        // Always seed, even for a brand-new signature with no body: the call also carries the
        // placeholder, and the bundle's default ("Write your message") is the composer's wording,
        // which is a lie here. Doing it at page-ready, not right after the load, is what
        // guarantees window.setSignatureBody exists.
        // The toolbar's strings first, then the body, setSignatureBody carries this surface's own
        // placeholder and must win over the composer wording setComposerLabels sends.
        host.PageReady = async () =>
        {
            await host.RunAsync(ComposerLabels.Script());
            // Only the page's colours and the toolbar's sun or moon: a link dialog cannot open over
            // this one, so the editor keeps its own link panel.
            await host.RunAsync(MailcalBindingsMethods.ComposerPageRequestsScript());
            await host.RunAsync(
                $"window.setSignatureBody({EditorWebViewHost.Arg(editing.BodyHtml)}, "
                + $"{EditorWebViewHost.Arg(L10n.SettingsSignaturesPlaceholder())})");
            // Writing the signature is the only thing this screen is for, so the caret opens in it.
            // Asked for rather than assumed: the shared bundle focuses nothing of its own accord,
            // because in the composer the caret belongs in To (docs/contacts.md §4).
            view.Focus(FocusState.Programmatic);
            await host.RunAsync("window.focusComposerBody()");
        };
        _ = LoadSignatureEditorAsync(host, said);

        panel.Children.Add(parts.ImageSlot);
        panel.Children.Add(said);

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Right,
            Margin = new Thickness(0, 8, 0, 0),
        };
        var cancel = new Button { Content = L10n.ActionCancel() };
        cancel.Click += (_, _) => Apply(() => _editingSignature = null);
        buttons.Children.Add(cancel);
        buttons.Children.Add(parts.SaveSlot);
        panel.Children.Add(buttons);

        nameBox.TextChanged += (_, _) => DrawSignatureControls(parts);
        DrawSignatureControls(parts);
        return panel;
    }

    // The page moved while the editor is open: act on what a change came to, once, then draw its
    // controls from the page. What is typed stays where it is.
    private void RefreshSignatureEditor(EditingSignature editing, SignaturesPage page)
    {
        if (_signatureParts is not { } parts || parts.Editing != editing)
        {
            return;
        }
        parts.Page = page;
        switch (editing.Session.Read(page.Outcome, out var reason))
        {
            case SignatureEditorStep.Close:
                Apply(() => _editingSignature = null);
                return;
            case SignatureEditorStep.Refused:
                if (reason is not null)
                {
                    Show(parts.Said, SignatureEditorSession.Say(reason, SignatureRefusalText()));
                }
                break;
        }
        DrawSignatureControls(parts);
    }

    // Save and Add image from the page, the name box held with them while a change runs. Save is
    // held besides while the core refuses the name typed.
    private void DrawSignatureControls(SignatureEditorParts parts)
    {
        ControlComponents.HoldFields(parts.Page.Save, parts.Name);
        var save = SignatureEditorSession.SaveState(parts.Page.Save, MailboxModel.NameRefusal(parts.Name.Text));
        if (save != parts.SaveDrawn)
        {
            parts.SaveDrawn = save;
            parts.SaveSlot.Child = ControlComponents.Button(
                L10n.SettingsSignaturesSave(),
                save,
                "SignatureSave",
                () => _ = SaveSignatureAsync(parts),
                (Style)Application.Current.Resources["AccentButtonStyle"]);
        }
        // Add image starts nothing in the core, so it never runs; it waits while a change does.
        var addImage = parts.Page.Save with { Running = false };
        if (addImage != parts.ImageDrawn)
        {
            parts.ImageDrawn = addImage;
            parts.ImageSlot.Child = ControlComponents.Button(
                L10n.SettingsSignaturesInsertImage(),
                addImage,
                "SignatureInsertImage",
                () => _ = InsertSignatureImageAsync(parts));
        }
    }

    private async Task LoadSignatureEditorAsync(EditorWebViewHost host, TextBlock said)
    {
        try
        {
            await host.LoadAsync();
        }
        catch (Exception ex)
        {
            Log.Warn($"signatures: couldn't load the WebView2 editor ({ex.GetType().Name})");
            Show(said, L10n.SettingsSignaturesImageFailed());
        }
    }

    // Reads back what the user authored and asks the core to store it under the editor's id, with
    // the name as typed: the core trims it and refuses a blank one.
    private async Task SaveSignatureAsync(SignatureEditorParts parts)
    {
        parts.Said.Visibility = Visibility.Collapsed;
        string? body;
        try
        {
            body = await parts.Host.ReadStringAsync("window.signatureBody()");
        }
        catch (Exception ex)
        {
            Log.Warn($"signatures: couldn't read the authored body ({ex.GetType().Name})");
            return;
        }
        if (SignatureDraft.Parse(body) is not { } draft)
        {
            // The bundle has not parsed yet. Leave the editor open rather than storing an empty body
            // over a signature the user was editing.
            Log.Warn("signatures: save ignored, the editor has not loaded yet");
            return;
        }
        _model.ChangeSignatures(new SignatureIntent.Save(
            parts.Editing.Session.Id, parts.Name.Text, draft.BodyHtml, draft.BodyPlain));
    }

    // Offers PNG, JPEG and GIF, asks the core about the picked file's type and size before reading
    // it, and inserts it at the caret.
    private async Task InsertSignatureImageAsync(SignatureEditorParts parts)
    {
        parts.Said.Visibility = Visibility.Collapsed;
        var picker = new FileOpenPicker(PickerOwner.For(parts.Said));
        foreach (var extension in new[] { ".png", ".jpg", ".jpeg", ".gif" })
        {
            picker.FileTypeFilter.Add(extension);
        }
        var path = (await picker.PickSingleFileAsync())?.Path;
        if (string.IsNullOrEmpty(path))
        {
            return;
        }
        StorageFile file;
        ulong size;
        try
        {
            file = await StorageFile.GetFileFromPathAsync(path);
            size = (await file.GetBasicPropertiesAsync()).Size;
        }
        catch (Exception ex)
        {
            Log.Warn($"signature image unreadable: {ex.GetType().Name}");
            Show(parts.Said, L10n.SettingsSignaturesImageFailed());
            return;
        }
        if (MailboxModel.ImageRefusal(file.ContentType, size) is { } refusal)
        {
            Show(parts.Said, SignatureEditorSession.Say(refusal, SignatureRefusalText()));
            return;
        }
        byte[] bytes;
        try
        {
            bytes = await File.ReadAllBytesAsync(file.Path);
        }
        catch (Exception ex)
        {
            Log.Warn($"signatures: couldn't read the picked image ({ex.GetType().Name})");
            Show(parts.Said, L10n.SettingsSignaturesImageFailed());
            return;
        }
        // The editor closed while the picker was open: its WebView2 is gone.
        if (_signatureParts != parts)
        {
            return;
        }
        var payload = SignatureImage.Payload(
            SignatureImage.DataUrl(bytes, file.ContentType), Path.GetFileNameWithoutExtension(file.Name));
        await parts.Host.RunAsync($"window.insertSignatureImage({EditorWebViewHost.Arg(payload)})");
    }

    // The catalog's words for a refused signature or picture.
    private static SignatureRefusalWords SignatureRefusalText() => new(
        L10n.SettingsSignaturesNameRequired(),
        L10n.SettingsSignaturesGone(),
        L10n.SettingsSignaturesImageFormat(),
        L10n.SettingsSignaturesImageTooLarge);

    private static void Show(TextBlock target, string message)
    {
        target.Text = message;
        target.Visibility = Visibility.Visible;
    }

    // The open editor: what it edits, its host and fields, the page it last drew and the two slots
    // its page-drawn buttons sit in.
    private sealed class SignatureEditorParts(
        EditingSignature editing, EditorWebViewHost host, TextBox name, TextBlock said, SignaturesPage page)
    {
        internal EditingSignature Editing { get; } = editing;

        internal EditorWebViewHost Host { get; } = host;

        internal TextBox Name { get; } = name;

        internal TextBlock Said { get; } = said;

        internal SignaturesPage Page { get; set; } = page;

        internal Border SaveSlot { get; } = new();

        internal Border ImageSlot { get; } = new() { HorizontalAlignment = HorizontalAlignment.Left };

        internal ControlState? SaveDrawn { get; set; }

        internal ControlState? ImageDrawn { get; set; }
    }

    // What window.signatureBody() hands back: the HTML to store and its plain-text rendering.
    private sealed record SignatureDraft(string BodyHtml, string BodyPlain)
    {
        internal static SignatureDraft? Parse(string? json)
        {
            if (string.IsNullOrWhiteSpace(json))
            {
                return null;
            }
            try
            {
                using var parsed = System.Text.Json.JsonDocument.Parse(json);
                var root = parsed.RootElement;
                if (!root.TryGetProperty("body_html", out var html))
                {
                    return null;
                }
                var plain = root.TryGetProperty("body_plain", out var value) ? value.GetString() : null;
                return new SignatureDraft(html.GetString() ?? string.Empty, plain ?? string.Empty);
            }
            catch (System.Text.Json.JsonException)
            {
                return null;
            }
        }
    }
}
