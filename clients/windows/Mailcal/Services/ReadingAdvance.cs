// Where the reading pane lands when the message in it leaves the folder, the Windows half of the
// same rule the Apple client applies (clients/apple/.../Mailcal.AutoAdvance.swift).
//
// Archiving or deleting from the reading view used to empty the pane, so working through a mailbox
// meant going back to the list and clicking the next message every single time. It now opens the
// next one down, and, when the message was the last in the list, the one above it, so clearing out
// the bottom of a folder doesn't strand the reader on a placeholder with a full mailbox beside it.
//
// **Pure BCL on purpose, no WinUI.** That is what lets Mailcal.Tests link this file
// and gate the rule on every PR (the test project is a plain net10.0 assembly; a WinUI type reaching
// one of its linked files stops it compiling). The projection from the list's rows into `MessageStop`
// is the WinUI-facing half and stays in MailboxModel.Reading.cs, mirroring Apple, where only
// `messageAfterRemoving` is pure and `readableStops` lives on the view.

using System.Collections.Generic;
using Allodia.Mailcal.ViewModels;

namespace Allodia.Mailcal.Services;

/// <summary>
/// One message as the list displays it, enough to identify it and to fill the reading header.
/// </summary>
/// <remarks>
/// Plain strings, plus the sender's <see cref="AvatarItem"/>: the pane the auto-advance lands on
/// has to draw the same face the row it came from did, and AvatarItem is itself WinUI-free, so
/// carrying one costs this file nothing.
/// </remarks>
public readonly record struct MessageStop(
    string Account,
    string Key,
    string Subject,
    string From,
    AvatarItem Avatar,
    string DateText);

/// <summary>Chooses the message the reading pane should fall to when the open one is removed.</summary>
public static class ReadingAdvance
{
    /// <summary>
    /// The message to open once <paramref name="removed"/> is gone: the next one down, else the one
    /// above, else null.
    /// </summary>
    /// <remarks>
    /// <paramref name="stops"/> is the list <em>as it is on screen right now</em>, so this must be
    /// called before the archive/delete is dispatched, while <paramref name="removed"/> is still in
    /// it. A message that isn't in the list answers null, which empties the pane as before.
    /// </remarks>
    public static MessageStop? Next(MessageStop removed, IReadOnlyList<MessageStop> stops)
    {
        // Both halves must match: a provider key is unique only within its account, so two accounts
        // can mint the same one and matching on the key alone advances into the wrong mailbox.
        var index = -1;
        for (var i = 0; i < stops.Count; i++)
        {
            if (stops[i].Account == removed.Account && stops[i].Key == removed.Key)
            {
                index = i;
                break;
            }
        }

        if (index < 0)
        {
            return null;
        }

        if (index + 1 < stops.Count)
        {
            return stops[index + 1];
        }

        return index > 0 ? stops[index - 1] : null;
    }
}
