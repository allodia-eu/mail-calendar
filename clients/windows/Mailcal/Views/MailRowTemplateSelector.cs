// Picks the flat vs threaded row template, mirroring the macOS list's switch over
// SnapshotRow: a single message is interactive (context menu of reply/forward/flag/delete),
// while a conversation summary is display-only (its key is a thread id, not a message key,
// so the per-message actions don't apply).

using Allodia.Mailcal.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allodia.Mailcal.Views;

/// <summary>Chooses the flat-message or threaded-conversation row template.</summary>
public sealed partial class MailRowTemplateSelector : DataTemplateSelector
{
    /// <summary>Template for a single message (with the context menu).</summary>
    public DataTemplate? Flat { get; set; }

    /// <summary>Template for a conversation summary (display only).</summary>
    public DataTemplate? Thread { get; set; }

    /// <inheritdoc/>
    protected override DataTemplate? SelectTemplateCore(object item) =>
        item is MailRow { IsThread: true } ? Thread : Flat;

    /// <inheritdoc/>
    protected override DataTemplate? SelectTemplateCore(object item, DependencyObject container) =>
        SelectTemplateCore(item);
}
