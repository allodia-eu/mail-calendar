// The mark an account's row carries while a notice is about that account (docs/notices.md rule 6).
// WinUI-free, so Mailcal.Tests links it beside SidebarTree.

namespace Allodia.Mailcal.ViewModels;

/// <summary>A notice severity as the pane draws it: the generated enum is internal to the
/// bindings, and the pane's view models are public.</summary>
public enum SidebarMarkTone
{
    /// <summary>Something the account is relied on for has stopped.</summary>
    Blocked,

    /// <summary>Part of it works.</summary>
    Degraded,

    /// <summary>Nothing is broken.</summary>
    Informational,
}

/// <summary>An account's mark: a glyph by <paramref name="Tone"/>, <paramref name="Label"/> drawn
/// beneath the address, and <paramref name="Detail"/>, every one of its notices, for a screen
/// reader.</summary>
public sealed record SidebarMark(SidebarMarkTone Tone, string Label, string Detail)
{
    /// <summary>The mark's glyph.</summary>
    public string Glyph => GlyphFor(Tone);

    /// <summary>The Segoe Fluent glyph for <paramref name="tone"/> (docs/icons.md): Error, Warning
    /// or Info, wherever a notice is drawn with one.</summary>
    public static string GlyphFor(SidebarMarkTone tone) => tone switch
    {
        SidebarMarkTone.Blocked => "\uE783",
        SidebarMarkTone.Degraded => "\uE7BA",
        _ => "\uE946",
    };
}
