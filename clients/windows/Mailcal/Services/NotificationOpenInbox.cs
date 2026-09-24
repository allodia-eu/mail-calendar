// The message a clicked new-mail notification named, and where one waits when it arrives before
// there is a window to open it in (docs/background-sync.md).
//
// Two shapes of click reach this app and they arrive by different routes. A click while it is
// running is a COM activation into the live process, which AppNotificationManager raises as
// NotificationInvoked (NewMailNotifier.Arm). A click that STARTS the app arrives as the launch's
// own activation arguments instead, in Main, where there is no window and no core yet: that one is
// parked here and drained once the shell is up, exactly as a cold-start mail link is
// (MailLinkInbox).

using System;
using System.Buffers.Text;
using System.Collections.Generic;
using System.Text;

namespace Allodia.Mailcal.Services;

/// <summary>The message a notification names: the pair the core keys mail on.</summary>
/// <param name="Account">The account id the message belongs to.</param>
/// <param name="MessageKey">The message's stable provider key.</param>
/// <remarks>
/// A key is unique within its account and never across them, which is why a notification carries
/// the pair rather than the key alone.
/// </remarks>
internal readonly record struct NotificationTarget(string Account, string MessageKey)
{
    /// <summary>The toast-argument key carrying <see cref="Account"/>.</summary>
    internal const string AccountArgument = "account";

    /// <summary>The toast-argument key carrying <see cref="MessageKey"/>.</summary>
    internal const string MessageArgument = "message";

    /// <summary>What this message's two toast arguments should be set to.</summary>
    internal (string Account, string Message) Arguments =>
        (Encode(Account), Encode(MessageKey));

    /// <summary>
    /// The message a toast's arguments name, or <c>null</c> where they name none.
    /// </summary>
    /// <remarks>
    /// Both halves or neither. A pair with one side missing resolves to no message at all, so
    /// acting on it would open the app looking as if the click went to the wrong message rather
    /// than to none; and the summary deliberately carries neither, which is what makes it open the
    /// app alone. Reading and writing share the two key constants above, because a key typo'd on
    /// one side is a click that silently stops opening anything.
    /// </remarks>
    internal static NotificationTarget? From(IDictionary<string, string> arguments) =>
        arguments.TryGetValue(AccountArgument, out var account)
            && arguments.TryGetValue(MessageArgument, out var message)
            && Decode(account) is { Length: > 0 } decodedAccount
            && Decode(message) is { Length: > 0 } decodedMessage
            ? new NotificationTarget(decodedAccount, decodedMessage)
            : null;

    // A launch argument list is `key=value;key=value`, so a value carrying `=` or `;` is a value
    // that changes where the list is cut. A provider key is arbitrary text and Microsoft Graph's
    // are base64 WITH padding, so they carry `=` routinely.
    //
    // Base64url puts every value in [A-Za-z0-9-_], which no separator and no escape appears in.
    // That is deliberately belt and braces: the builder is documented to encode what it is given,
    // and this holds whether or not it does, on a path no test here can reach and whose failure is
    // one provider's notifications quietly opening nothing.
    private static string Encode(string value) =>
        Base64Url.EncodeToString(Encoding.UTF8.GetBytes(value));

    private static string? Decode(string value)
    {
        try
        {
            return Encoding.UTF8.GetString(Base64Url.DecodeFromChars(value));
        }
        catch (FormatException)
        {
            // Not ours, or damaged in transit: the click then names no message and opens the app,
            // which is what it did before there was a deep link at all.
            return null;
        }
    }
}

/// <summary>The clicked notification a cold start carried, until the shell can open it.</summary>
internal static class NotificationOpenInbox
{
    private static NotificationTarget? _pending;
    private static readonly object Gate = new();

    /// <summary>
    /// The message waiting to be opened, or <c>null</c>. Set once in <c>Main</c>, taken once by
    /// the shell.
    /// </summary>
    /// <remarks>
    /// Behind a lock rather than <c>Volatile</c>: the value is a struct wider than a word, so a
    /// torn read is a real outcome here and would hand the shell one message's account with
    /// another's key, which resolves to nothing at all.
    /// </remarks>
    internal static NotificationTarget? Pending
    {
        get
        {
            lock (Gate)
            {
                return _pending;
            }
        }
        set
        {
            lock (Gate)
            {
                _pending = value;
            }
        }
    }

    /// <summary>Takes the waiting message, leaving none, so a click is only answered once.</summary>
    internal static NotificationTarget? Take()
    {
        lock (Gate)
        {
            var waiting = _pending;
            _pending = null;
            return waiting;
        }
    }
}
