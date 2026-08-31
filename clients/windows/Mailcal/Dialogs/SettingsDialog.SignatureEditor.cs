// The signature body editor (Settings ▸ Signatures ▸ Edit): the shared clients/composer/dist/editor.html
// bundle hosted body-only, through the SAME EditorWebViewHost the composer uses, one definition of
// the gates, so the two cannot drift (docs/composer-security.md, docs/signatures.md). Authoring a
// signature is authoring mail content, so it gets the composer's gates, not a lighter set.
//
// It renders INSIDE the settings detail panel rather than in a dialog of its own, because a nested
// ContentDialog is not allowed, the same constraint that made the destructive database reset
// confirm in place.
//
// The one thing it does that the composer does not is insert an image as a self-contained `data:`
// URI. That is what a signature stores (one file, no side-car blobs to lose) and what the core
// rewrites to a `cid:` part on send, because Outlook's reader blocks `data:` images.

using Allodia.Mailcal.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Storage;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace Allodia.Mailcal.Dialogs;

public sealed partial class SettingsDialog
{
    // The live editor host, held so the WebView2 can be released when the panel is rebuilt or the
    // dialog closes. A WebView2 dropped from the tree is not disposed by dropping it.
    private EditorWebViewHost? _signatureEditor;

    /// <summary>Releases the signature editor's WebView2, if one is up. Called before every detail
    /// rebuild and when the dialog closes.</summary>
    private void CloseSignatureEditor()
    {
        _signatureEditor?.Close();
        _signatureEditor = null;
    }

    private UIElement BuildSignatureEditor(EditingSignature editing)
    {
        var panel = new StackPanel { Spacing = 8 };
        panel.Children.Add(Heading(editing.Id is null ? L10n.SettingsSignaturesAdd() : editing.Name));

        // A signature with no name is a row the user cannot tell apart in the picker, so Save waits
        // for one. Declared before the box so the TextChanged handler below can gate it.
        var save = new Button
        {
            Content = L10n.SettingsSignaturesSave(),
            Style = (Style)Application.Current.Resources["AccentButtonStyle"],
            IsEnabled = !string.IsNullOrWhiteSpace(editing.Name),
        };

        var nameBox = new TextBox
        {
            Header = L10n.SettingsSignaturesNameLabel(),
            PlaceholderText = L10n.SettingsSignaturesNamePlaceholder(),
            Text = editing.Name,
            MinWidth = 240,
            HorizontalAlignment = HorizontalAlignment.Left,
        };
        nameBox.TextChanged += (_, _) => save.IsEnabled = !string.IsNullOrWhiteSpace(nameBox.Text);
        panel.Children.Add(nameBox);

        panel.Children.Add(new TextBlock
        {
            Text = L10n.SettingsSignaturesBodyLabel(),
            Margin = new Thickness(0, 8, 0, 0),
        });

        // An explicit height: the detail panel lives in a ScrollViewer, which offers infinite height
        // during measure, and a WebView2 given that lays out at zero.
        var view = new WebView2();
        panel.Children.Add(new Border
        {
            BorderBrush = (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources["ControlStrokeColorDefaultBrush"],
            BorderThickness = new Thickness(1),
            Height = 230,
            Child = view,
        });

        var imageError = new TextBlock
        {
            Visibility = Visibility.Collapsed,
            TextWrapping = TextWrapping.Wrap,
            Foreground = (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources["SystemFillColorCriticalBrush"],
            Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"],
        };

        var host = new EditorWebViewHost(view);
        _signatureEditor = host;
        // Always seed, even for a brand-new signature with no body: the call also carries the
        // placeholder, and the bundle's default ("Write your message") is the composer's wording,
        // which is a lie here. Doing it at page-ready, not right after the load, is what
        // guarantees window.setSignatureBody exists.
        // The toolbar's strings first, then the body, setSignatureBody carries this surface's own
        // placeholder and must win over the composer wording setComposerLabels sends.
        host.PageReady = async () =>
        {
            await host.RunAsync(ComposerLabels.Script());
            await host.RunAsync(
                $"window.setSignatureBody({EditorWebViewHost.Arg(editing.BodyHtml)}, "
                + $"{EditorWebViewHost.Arg(L10n.SettingsSignaturesPlaceholder())})");
            // Writing the signature is the only thing this screen is for, so the caret opens in it.
            // Asked for rather than assumed: the shared bundle focuses nothing of its own accord,
            // because in the composer the caret belongs in To (docs/contacts.md §4).
            view.Focus(FocusState.Programmatic);
            await host.RunAsync("window.focusComposerBody()");
        };
        _ = LoadSignatureEditorAsync(host, imageError);

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var addImage = new Button { Content = L10n.SettingsSignaturesInsertImage() };
        addImage.Click += async (_, _) => await InsertSignatureImageAsync(host, imageError);
        actions.Children.Add(addImage);
        panel.Children.Add(actions);
        panel.Children.Add(imageError);

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Right,
            Margin = new Thickness(0, 8, 0, 0),
        };
        var cancel = new Button { Content = L10n.ActionCancel() };
        cancel.Click += (_, _) => Apply(() => _editingSignature = null);
        save.Click += async (_, _) => await SaveSignatureAsync(host, editing, nameBox.Text);
        buttons.Children.Add(cancel);
        buttons.Children.Add(save);
        panel.Children.Add(buttons);

        return panel;
    }

    private async Task LoadSignatureEditorAsync(EditorWebViewHost host, TextBlock imageError)
    {
        try
        {
            await host.LoadAsync();
        }
        catch (Exception ex)
        {
            Log.Warn($"signatures: couldn't load the WebView2 editor ({ex.GetType().Name})");
            Show(imageError, L10n.SettingsSignaturesImageFailed());
        }
    }

    // Reads back what the user authored and stores it. The caller decided whether this is a create
    // or an update when it opened the editor (it knows which signature that was).
    private async Task SaveSignatureAsync(EditorWebViewHost host, EditingSignature editing, string name)
    {
        string? body;
        try
        {
            body = await host.ReadStringAsync("window.signatureBody()");
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
        var trimmed = name.Trim();
        if (editing.Id is { } id)
        {
            _model.UpdateSignature(id, trimmed, draft.BodyHtml, draft.BodyPlain);
        }
        else
        {
            _model.CreateSignature(trimmed, draft.BodyHtml, draft.BodyPlain);
        }
        Apply(() => _editingSignature = null);
    }

    private async Task InsertSignatureImageAsync(EditorWebViewHost host, TextBlock imageError)
    {
        imageError.Visibility = Visibility.Collapsed;
        var picker = new FileOpenPicker();
        foreach (var extension in new[] { ".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp" })
        {
            picker.FileTypeFilter.Add(extension);
        }
        if (App.MainWindow is not null)
        {
            InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(App.MainWindow));
        }
        var file = await picker.PickSingleFileAsync();
        if (file is null || string.IsNullOrEmpty(file.Path))
        {
            return;
        }
        switch (await ReadSignatureImageAsync(file))
        {
            case SignatureImageOutcome.DataUrl picked:
                await host.RunAsync(
                    "window.insertSignatureImage("
                    + EditorWebViewHost.Arg(SignatureImagePayload(picked))
                    + ")");
                break;
            case SignatureImageOutcome.TooLarge tooLarge:
                Show(imageError, L10n.SettingsSignaturesImageTooLarge(
                    SignatureImage.FormatLimit(tooLarge.LimitBytes)));
                break;
            default:
                Show(imageError, L10n.SettingsSignaturesImageFailed());
                break;
        }
    }

    // Reads at most one byte past the cap: the user may pick a 4 GB file from a cloud provider, and
    // pulling all of it into memory to then refuse it is not a thing to do on the UI thread.
    private static async Task<SignatureImageOutcome> ReadSignatureImageAsync(StorageFile file)
    {
        try
        {
            var buffer = new byte[SignatureImage.LimitBytes + 1];
            int read;
            await using (var stream = File.OpenRead(file.Path))
            {
                read = await stream.ReadAtLeastAsync(buffer, buffer.Length, throwOnEndOfStream: false);
            }
            return SignatureImage.From(
                buffer.AsSpan(0, read).ToArray(),
                string.IsNullOrWhiteSpace(file.ContentType) ? null : file.ContentType,
                Path.GetFileNameWithoutExtension(file.Name));
        }
        catch (Exception ex)
        {
            Log.Warn($"signatures: couldn't read the picked image ({ex.GetType().Name})");
            return new SignatureImageOutcome.Failed();
        }
    }

    // The insertSignatureImage payload. The alt text rides as a property the editor assigns to
    // node.alt, so it cannot carry markup into the attribute.
    private static string SignatureImagePayload(SignatureImageOutcome.DataUrl image) =>
        System.Text.Json.JsonSerializer.Serialize(new Dictionary<string, string>
        {
            ["data_url"] = image.Value,
            ["alt_text"] = image.AltText,
        });

    private static void Show(TextBlock target, string message)
    {
        target.Text = message;
        target.Visibility = Visibility.Visible;
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
