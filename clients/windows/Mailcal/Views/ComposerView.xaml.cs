// The rich composer, hosted in the reading-pane slot (it used to be a ContentDialog, which
// blacked out the mailbox behind it). It hosts only the bundled editor asset in a
// dedicated WebView2, then asks Rust to validate/render/queue the composer document.
//
// THE WEBVIEW2 GATES THIS USES ARE A CROSS-PLATFORM CONTRACT, see docs/composer-security.md,
// "Layer 3, Native WebView host gates". They no longer live here: they are EditorWebViewHost
// (Services/EditorWebView.cs), which the Settings signature editor hosts the same bundle through,
// so there is one definition and the two cannot drift, the same collapse Android made into
// EditorWebView.kt. Do not relax one without updating that doc (rule AND matrix) and every other
// platform.

using Allodia.Mailcal.Dialogs;
using Allodia.Mailcal.Services;
using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using uniffi.mailcal_bindings;
using Windows.Storage;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace Allodia.Mailcal.Views;

/// <summary>A rich HTML composer for a new message, reply, reply-all, or forward, rendered in the
/// detail column in place of the reading pane. Every kind exposes editable To/Cc/Bcc fields,
/// reply/reply-all open with To/Cc pre-filled from the core, new/forward open empty, and only a
/// new message edits the Subject. Every kind shares the one hardened editor host.</summary>
public sealed partial class ComposerView : UserControl
{
    private readonly List<PickedComposerAttachment> _attachments = new();
    private MailboxModel? _model;
    private ComposeRequest? _request;
    private Action? _onDone;

    // The hardened host of the shared editor bundle, the same one the Settings signature editor
    // uses. Built here rather than in Init so the field is never null.
    private readonly EditorWebViewHost _editor;

    /// <summary>The editor document as it stood once the bundle had loaded and the quoted original
    /// (if any) had been seeded, the "user hasn't typed anything yet" baseline the discard prompt
    /// compares against. Null until the editor is ready, at which point nothing can have been typed
    /// into it yet.</summary>
    private string? _seedDocument;

    /// <summary>Whether the user has edited a header field (To/Cc/Bcc/Subject) since the composer
    /// opened. Tracked eagerly because it needs no round-trip into the editor.</summary>
    private bool _headersDirty;

    /// <summary>Whether the editor bundle has parsed, so its <c>window.*</c> hooks exist. Anything
    /// the user can trigger that talks to the editor gates on this.</summary>
    private bool _editorReady;

    /// <summary>Initialises the control.</summary>
    public ComposerView()
    {
        this.InitializeComponent();
        _editor = new EditorWebViewHost(Editor) { PageReady = OnEditorReadyAsync };
    }

    /// <summary>Binds the composer to a request and starts loading the editor. <paramref name="onDone"/>
    /// is invoked once the composer is finished, after a successful send, or on Cancel, and the
    /// shell restores the reading pane.</summary>
    internal void Init(MailboxModel model, ComposeRequest request, Action onDone)
    {
        _model = model;
        _request = request;
        _onDone = onDone;

        TitleText.Text = request.Title;
        // The From dropdown opens on the request's account, falling back to the first configured
        // one, so the shown sender is always the one the send will actually go out as (a blank From
        // field would be the failure the picker exists to prevent). With one account there is
        // nothing to choose, so the read-only row shows in its place; the dropdown is still
        // populated and selected behind it, because OnSend reads the account off it either way.
        foreach (var account in model.Accounts)
        {
            FromBox.Items.Add(account);
        }
        FromBox.SelectedItem = model.Accounts.FirstOrDefault(a => a.Id == request.InitialFrom)
            ?? model.Accounts.FirstOrDefault();
        if (model.Accounts.Count <= 1)
        {
            FromBox.Visibility = Visibility.Collapsed;
            FromRow.Visibility = Visibility.Visible;
            FromAddress.Text = (FromBox.SelectedItem as AccountItem)?.Email ?? string.Empty;
        }

        // The three recipient fields. Autosuggest is wired before the prefill so a reply's To is
        // rendered by a field that already knows how to complete the next one; the lookup itself
        // hops off the UI thread inside the model.
        foreach (var (field, label, automationId) in new[]
                 {
                     (ToField, L10n.ComposeTo(), "ToField"),
                     (CcField, L10n.ComposeCc(), "CcField"),
                     (BccField, L10n.ComposeBcc(), "BccField"),
                 })
        {
            field.Label = label;
            field.InputAutomationId = automationId;
            field.SuggestionsFor = model.RecipientSuggestionsAsync;
            field.RecipientsChanged += OnRecipientsChanged;
        }
        // Seeded, not assigned raw: every address the request carries is finished, so all of them
        // render as pills. The field's own rule reads whatever follows the last comma as the token
        // the user is typing, which is right for a keystroke and wrong for a pre-fill, it left a
        // reply-all's last recipient as loose text beside one pill (RecipientTokens.Seeded).
        ToField.Text = RecipientTokens.Seeded(request.InitialTo);
        CcField.Text = RecipientTokens.Seeded(request.InitialCc);
        // Bcc and Subject are pre-filled only by an assistant's draft (docs/mcp.md); every other
        // path leaves them at the empty defaults. Both are set before the dirty tracking is armed
        // below, so arriving prefilled is the request's doing rather than the user's.
        BccField.Text = RecipientTokens.Seeded(request.InitialBcc);
        SubjectBox.Text = request.InitialSubject;
        SubjectBox.Visibility = request.ShowsSubject ? Visibility.Visible : Visibility.Collapsed;
        // Cc and Bcc open collapsed, but never over a recipient the request put there. A reply-all
        // fills Cc, and a mail link may name a Bcc, and a recipient the sender cannot see is one
        // they cannot remove (docs/composer-security.md, Gate 12). Applied rather than left to the
        // Checked handler: assigning `false` to something already unchecked raises nothing.
        CcBccToggle.IsChecked = RecipientTokens.RevealsCcBcc(request.InitialCc, request.InitialBcc);
        ApplyCcBcc();
        // Send is gated on a non-empty To, exactly as the dialog's PrimaryButton was.
        SendButton.IsEnabled = !string.IsNullOrWhiteSpace(ToField.Text);

        // The style toggle shows only when there is a quoted original AND the user opted into
        // per-message styling in Settings, off by default, so an ordinary reply shows no picker
        // and just uses the app default. Its initial selection reflects that default; the editor
        // isn't ready yet, so the Checked handler below no-ops (it guards on a null CoreWebView2)
        // until the page has loaded.
        if (request.ShowsStylePicker)
        {
            QuoteStylePanel.Visibility = Visibility.Visible;
            if (request.QuoteStyle == QuoteStyleChoice.LineAndHeader)
            {
                QuoteLineHeaderRadio.IsChecked = true;
            }
            else
            {
                QuoteIndentedRadio.IsChecked = true;
            }
        }

        // The Signature control and the library behind it (ComposerView.Signature.cs). Built before
        // the editor loads, because the page-ready seeding reads the resolved signature off it.
        InitSignatures();

        // Pre-filled recipients are the request's doing, not the user's, arm the dirty tracking
        // only once they are in place, so a reply doesn't open already "dirty".
        _headersDirty = false;
        _ = LoadEditorAsync();
    }

    /// <summary>
    /// Whether the user has changed anything since the composer opened, the question the
    /// "Discard draft?" prompt turns on. True as soon as a header field is edited; otherwise the
    /// editor document is compared against the seed it opened with, so a reply that merely carries
    /// its quoted original does NOT count as dirty until something is actually written above it
    /// (and flipping the quote-style toggle, which rewrites the document, does).
    /// </summary>
    internal async Task<bool> IsDirtyAsync()
    {
        if (_headersDirty || _attachments.Count > 0)
        {
            return true;
        }
        // No seed yet means the editor bundle hasn't finished loading, so nothing can have been
        // typed into it. Treat that as clean rather than blocking the user behind a prompt.
        if (_seedDocument is null || _editor.Core is null)
        {
            return false;
        }
        try
        {
            return await ReadDocumentAsync() != _seedDocument;
        }
        catch (Exception ex)
        {
            // Can't tell, err toward keeping the draft (prompt), never toward silently dropping it.
            Log.Warn($"composer: couldn't read the document to check for edits ({ex.GetType().Name})");
            return true;
        }
    }

    /// <summary>Tears the editor down. The composer is built fresh per draft rather than reused, so
    /// nothing, a document, a quote, an attachment list, can leak from one message into the next;
    /// this releases the WebView2 that backed it.</summary>
    internal void Teardown() => _editor.Close();

    private async void OnSend(object sender, RoutedEventArgs e)
    {
        if (_model is null || _request is null)
        {
            return;
        }
        SendButton.IsEnabled = false;
        try
        {
            var documentJson = await ReadDocumentAsync();
            // The pills are a rendering of these strings, never a second source of truth, so what
            // is submitted is exactly what the user can see in the fields.
            var recipients = new Recipients(ToField.Text, CcField.Text, BccField.Text);
            var files = _attachments.Select(a => a.File).ToArray();
            var from = (FromBox.SelectedItem as AccountItem)?.Id;
            if (string.IsNullOrEmpty(documentJson) || !Submit(recipients, documentJson, files, from))
            {
                PrepareError.Visibility = Visibility.Visible;
                SendButton.IsEnabled = !string.IsNullOrWhiteSpace(ToField.Text);
                return;
            }
            PrepareError.Visibility = Visibility.Collapsed;
            _onDone?.Invoke();
        }
        catch (Exception ex)
        {
            Log.Warn($"composer: couldn't prepare document ({ex.GetType().Name})");
            PrepareError.Visibility = Visibility.Visible;
            SendButton.IsEnabled = !string.IsNullOrWhiteSpace(ToField.Text);
        }
    }

    // Route the rendered document to the submit call this request is for. A reply/forward carries
    // the original's (account, key) so the core can derive the Re:/Fwd: subject and the threading;
    // `from` may name a different account, and the core still resolves the original in the one that
    // holds it, so a cross-account reply still threads.
    private bool Submit(Recipients recipients, string documentJson, ComposerFileAttachment[] files, string? from) =>
        _request! switch
        {
            { Kind: RichComposeKind.Forward, Account: { } account, Key: { } key } =>
                _model!.SubmitRichForward(account, key, recipients, documentJson, files, from),
            { Kind: RichComposeKind.Reply or RichComposeKind.ReplyAll, Account: { } account, Key: { } key } =>
                _model!.SubmitRichReply(account, key, recipients, documentJson, files, from),
            _ => _model!.SubmitRich(recipients, SubjectBox.Text, documentJson, files, from),
        };

    private void OnCancel(object sender, RoutedEventArgs e) => _onDone?.Invoke();

    private void OnToggleCcBcc(object sender, RoutedEventArgs e) => ApplyCcBcc();

    // Cc/Bcc follow the toggle, and the chevron points the way it would move them.
    private void ApplyCcBcc()
    {
        var revealed = CcBccToggle.IsChecked == true;
        CcBccPanel.Visibility = revealed ? Visibility.Visible : Visibility.Collapsed;
        CcBccChevron.Glyph = revealed ? "\uE70E" : "\uE70D";   // ChevronUp / ChevronDown
    }

    private void OnHeaderChanged(object sender, TextChangedEventArgs e)
    {
        _headersDirty = true;
        SendButton.IsEnabled = !string.IsNullOrWhiteSpace(ToField.Text);
    }

    // A recipient changed, typed, completed from a suggestion, or removed with its pill. The same
    // two consequences as any other header edit: the draft is dirty, and Send follows To.
    private void OnRecipientsChanged(object? sender, EventArgs e)
    {
        _headersDirty = true;
        SendButton.IsEnabled = !string.IsNullOrWhiteSpace(ToField.Text);
    }

    // The user flipped the per-message style toggle: re-style the quoted original in place. Until
    // the editor's WebView2 has loaded (including the programmatic check during Init), CoreWebView2
    // is null and this is a no-op.
    private async void OnQuoteStyleChanged(object sender, RoutedEventArgs e)
    {
        if (_editor.Core is null)
        {
            return;
        }
        var token = ComposerQuote.Token(
            QuoteLineHeaderRadio.IsChecked == true
                ? QuoteStyleChoice.LineAndHeader
                : QuoteStyleChoice.Indented);
        await _editor.RunAsync($"window.setComposerQuoteStyle({EditorWebViewHost.Arg(token)})");
    }

    private Task<string?> ReadDocumentAsync() => _editor.ReadStringAsync("composerDocument()");

    private async Task LoadEditorAsync()
    {
        try
        {
            await _editor.LoadAsync();
        }
        catch (Exception ex)
        {
            Log.Warn($"composer: couldn't load WebView2 editor ({ex.GetType().Name})");
            PrepareError.Visibility = Visibility.Visible;
        }
    }

    private async void OnAttachFiles(object sender, RoutedEventArgs e)
    {
        var picker = new FileOpenPicker();
        picker.FileTypeFilter.Add("*");
        if (App.MainWindow is not null)
        {
            InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(App.MainWindow));
        }
        var files = await picker.PickMultipleFilesAsync();
        foreach (var file in files)
        {
            if (string.IsNullOrEmpty(file.Path))
            {
                continue;
            }
            _attachments.Add(new PickedComposerAttachment(
                new ComposerFileAttachment(file.Path, file.Name, MediaType(file))));
        }
        AttachmentList.ItemsSource = null;
        AttachmentList.ItemsSource = _attachments;
    }

    private void OnRemoveAttachment(object sender, RoutedEventArgs e)
    {
        if (AttachmentList.SelectedItem is not PickedComposerAttachment selected)
        {
            return;
        }
        _attachments.Remove(selected);
        AttachmentList.ItemsSource = null;
        AttachmentList.ItemsSource = _attachments;
        RemoveAttachmentButton.IsEnabled = false;
    }

    private void OnAttachmentSelectionChanged(object sender, SelectionChangedEventArgs e) =>
        RemoveAttachmentButton.IsEnabled = AttachmentList.SelectedItem is not null;

    // The editor bundle has finished loading, so its window.* hooks now exist. Doing this here,
    // not right after the load, is what guarantees that; a hook called any earlier lands on an
    // undefined function and fails silently.
    //
    // The order is the contract. The signature goes LAST, after the quote: the editor decides where
    // to place it on first insert, above the quoted original when there is one, so seeding it
    // before the quote exists would put a reply's signature at the bottom, under the message it is
    // replying to. And the baseline snapshot goes after both, or a composer that merely opened with
    // what the account gave it would already count as edited.
    private async Task OnEditorReadyAsync()
    {
        // From here on the bundle's hooks are defined, which is what makes a live signature swap on
        // a From change safe to attempt (ComposerView.Signature.cs). Set before the seeding rather
        // than after it, so a seed that throws does not leave the sender silently unable to swap.
        _editorReady = true;
        try
        {
            // The chrome's own strings, before any content seed. They are independent of the seeds,
            // the placeholder lives on the editor element's dataset, which replacing the document
            // does not touch, but sending them first matches the other clients' open-time order.
            await _editor.RunAsync(ComposerLabels.Script());
            if (_request?.Quote is { } quote)
            {
                await _editor.RunAsync($"window.setComposerQuote({EditorWebViewHost.Arg(quote)})");
            }
            // An assistant's draft body, seeded in the quote's place and for the same reason ahead
            // of the signature: setPlainText assigns the WHOLE body, so anything injected first is
            // overwritten. Lengths only, never content (docs/logging.md), and not even that here,
            // since the seed is the assistant's text.
            if (_request?.InitialBody is { } body)
            {
                await _editor.RunAsync($"window.setPlainText({EditorWebViewHost.Arg(body)})");
            }
            await ApplySignatureAsync();
            _seedDocument = await ReadDocumentAsync();
            // The caret opens where the work starts, and here rather than earlier: after the seed
            // snapshot, never before, moving it must not be mistaken for the user having typed, or
            // the composer opens already dirty and prompts to discard on close.
            if (_request?.FocusesBody == true)
            {
                // A reply/forward already has its From/To/Subject filled in, so writing is the only
                // thing left to do. The WebView2 has to take focus itself as well as the DOM
                // element, or the caret sits in the editor while keystrokes go to the shell.
                Editor.Focus(FocusState.Programmatic);
                await _editor.RunAsync("window.focusComposerBody()");
            }
            else
            {
                // A new message the caller did not address: To is empty and is where the user has
                // to begin.
                ToField.FocusInput();
            }
        }
        catch (Exception ex)
        {
            // Leaves _seedDocument null: IsDirtyAsync then falls back to the header fields alone.
            Log.Warn($"composer: couldn't seed the editor ({ex.GetType().Name})");
        }
    }

    private static string MediaType(StorageFile file) =>
        string.IsNullOrWhiteSpace(file.ContentType) ? "application/octet-stream" : file.ContentType;

    private sealed class PickedComposerAttachment
    {
        public PickedComposerAttachment(ComposerFileAttachment file) => File = file;

        public ComposerFileAttachment File { get; }

        public string DisplayName => File.FileName;
    }
}
