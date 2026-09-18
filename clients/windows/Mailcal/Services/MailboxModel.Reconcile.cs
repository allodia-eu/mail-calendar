// The in-place reconcile every bound list here goes through, and why it is one rather than a
// Clear()+Add. Split from MailboxModel.Projection.cs to keep that file under the 500-line limit.
//
// A refresh that changes nothing must mutate nothing: each collection event is a container the
// framework has to realise, and the list also loses its scroll position, any open context menu and
// the focus a screen reader is holding. An account sync fires dozens of refreshes a second.

using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>
    /// Reconciles <paramref name="target"/> toward <paramref name="next"/> in place: items
    /// matched by <paramref name="key"/> are kept, gone items removed, new items inserted, and
    /// survivors moved into order, so unchanged rows keep their container. An item whose content
    /// changed is handed to <paramref name="update"/> where one is given, and replaced otherwise.
    /// </summary>
    /// <remarks>
    /// <paramref name="update"/> is what keeps a CHANGED row's container too. Replacing the item
    /// makes the ListView build a new container, and WinUI 3 reassigns focus when the element
    /// holding it goes, which activates that element's window: see MailRow.cs for the reading
    /// window that fell behind the mailbox because the message it opened had been marked read.
    /// A type with no change notifications has nothing to update with and still replaces.
    /// </remarks>
    private static void Reconcile<T>(
        ObservableCollection<T> target,
        IReadOnlyList<T> next,
        Func<T, string> key,
        Func<T, T, bool> equal,
        Action<T, T>? update = null)
    {
        var keep = new HashSet<string>(next.Select(key));
        for (var i = target.Count - 1; i >= 0; i--)
        {
            if (!keep.Contains(key(target[i])))
            {
                target.RemoveAt(i);
            }
        }
        // Earlier positions are already final, so the item for position i is at i or ahead.
        for (var i = 0; i < next.Count; i++)
        {
            var wanted = key(next[i]);
            if (i < target.Count && key(target[i]) == wanted)
            {
                Refresh(target, i, next[i], equal, update);
                continue;
            }
            var found = -1;
            for (var j = i + 1; j < target.Count; j++)
            {
                if (key(target[j]) == wanted)
                {
                    found = j;
                    break;
                }
            }
            if (found >= 0)
            {
                target.Move(found, i);
                Refresh(target, i, next[i], equal, update);
            }
            else
            {
                target.Insert(i, next[i]);
            }
        }
        while (target.Count > next.Count)
        {
            target.RemoveAt(target.Count - 1);
        }
    }

    // Brings the item at `index` up to date with `wanted`, in place where the type can be told to
    // update itself and by replacement where it cannot.
    private static void Refresh<T>(
        ObservableCollection<T> target,
        int index,
        T wanted,
        Func<T, T, bool> equal,
        Action<T, T>? update)
    {
        if (equal(target[index], wanted))
        {
            return;
        }
        if (update is null)
        {
            target[index] = wanted;
            return;
        }
        update(target[index], wanted);
    }
}
