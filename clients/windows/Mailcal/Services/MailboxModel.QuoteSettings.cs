// The quote-style surface of the model: projects the core's (internal) QuoteSettings into the
// public QuoteSettingsChoice the settings dialog and composer use, and forwards the setters to the
// Rust app. Split into its own partial to keep MailboxModel.cs under the 500-line limit. State
// lives in Rust (persisted); the core re-signals Surface.Settings after each setter.

using Allodia.Mailcal.ViewModels;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// The persisted reply/forward quoting settings, projected from the core, the default style,
    /// and whether the composer offers a per-message override of it. Falls back to the product
    /// default (indented, no per-message picker) before the app has connected. Read fresh each
    /// time (the core owns the value), so the settings dialog and composer reflect the latest.
    /// </summary>
    public QuoteSettingsChoice QuoteSettings
    {
        get
        {
            var settings = _app?.QuoteSettings();
            return settings is null
                ? new QuoteSettingsChoice(QuoteStyleChoice.Indented, PerMessage: false)
                : new QuoteSettingsChoice(Choice(settings.Style), settings.PerMessage);
        }
    }

    /// <summary>Sets and persists the default reply/forward quote style.</summary>
    public void SetQuoteStyleChoice(QuoteStyleChoice style) => _app?.SetQuoteStyle(Kind(style));

    /// <summary>Sets and persists whether the composer offers a per-message style override.</summary>
    public void SetQuoteStylePerMessage(bool perMessage) => _app?.SetQuoteStylePerMessage(perMessage);

    private static QuoteStyleChoice Choice(QuoteStyleKind kind) =>
        kind == QuoteStyleKind.LineAndHeader ? QuoteStyleChoice.LineAndHeader : QuoteStyleChoice.Indented;

    private static QuoteStyleKind Kind(QuoteStyleChoice choice) =>
        choice == QuoteStyleChoice.LineAndHeader ? QuoteStyleKind.LineAndHeader : QuoteStyleKind.Indented;
}
