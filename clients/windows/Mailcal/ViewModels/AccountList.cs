// How the model's account list moves onto a new snapshot. Its own file, WinUI-free, so that
// Mailcal.Tests can drive it through the same reconcile the pane runs.

using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;

namespace Allodia.Mailcal.ViewModels;

internal static class AccountList
{
    /// <summary>
    /// Moves <paramref name="target"/> onto <paramref name="wanted"/> in place: an account that
    /// left is removed, one that moved is moved, one whose drawing changed is replaced, one that
    /// arrived is inserted, and one that is <paramref name="same"/> raises nothing.
    /// </summary>
    /// <remarks>
    /// Never <c>Clear()</c> and refill. The folder pane reconciles on every event this collection
    /// raises, so after a <c>Clear()</c> it sees no accounts at all, takes every account row and
    /// its folder tree down, and builds them again as new, collapsed rows. That scrolls the pane
    /// under the reader and throws away every container it had realised, on every unread count
    /// that moves. Each state this method passes through is a list of real accounts, each once.
    /// </remarks>
    public static void Update(
        ObservableCollection<AccountItem> target,
        IReadOnlyList<AccountItem> wanted,
        Func<AccountItem, AccountItem, bool> same)
    {
        var keep = new HashSet<string>(wanted.Count);
        foreach (var account in wanted)
        {
            keep.Add(account.Id);
        }
        for (var i = target.Count - 1; i >= 0; i--)
        {
            if (!keep.Contains(target[i].Id))
            {
                target.RemoveAt(i);
            }
        }
        // Earlier positions are already final, so the account for position i is at i or ahead of it.
        for (var i = 0; i < wanted.Count; i++)
        {
            var found = -1;
            for (var j = i; j < target.Count; j++)
            {
                if (target[j].Id == wanted[i].Id)
                {
                    found = j;
                    break;
                }
            }
            if (found < 0)
            {
                target.Insert(i, wanted[i]);
                continue;
            }
            if (found != i)
            {
                target.Move(found, i);
            }
            if (!same(target[i], wanted[i]))
            {
                target[i] = wanted[i];
            }
        }
    }
}
