// Public, render-ready types for the reply/forward quote-style setting, the Windows mirror of the
// core's QuoteStyleKind / QuoteSettings. The generated UniFFI types are internal, so MailboxModel
// projects them into these public ones, keeping the FFI types confined to the service layer
// (mirroring SyncStrategyChoice in SyncSettingsViewModels.cs).

namespace Allodia.Mailcal.ViewModels;

/// <summary>How a quoted original is rendered below a reply or forward. Named for what each style
/// <em>is</em>, not for the mail client that popularized it.</summary>
public enum QuoteStyleChoice
{
    /// <summary>The original indented in a left-bordered blockquote under a one-line
    /// "On … wrote:" attribution.</summary>
    Indented,

    /// <summary>The original divided off with a top border and a
    /// <c>From:/Sent:/To:/Subject:</c> header block at full width.</summary>
    LineAndHeader,
}

/// <summary>The reply/forward quoting settings: the app-level default style, and whether the
/// composer offers a per-message override of it (off by default, an ordinary reply just uses the
/// default and shows no picker).</summary>
/// <param name="Style">The style a new reply or forward is seeded with.</param>
/// <param name="PerMessage">Whether the composer shows the style picker at all.</param>
public readonly record struct QuoteSettingsChoice(QuoteStyleChoice Style, bool PerMessage);
