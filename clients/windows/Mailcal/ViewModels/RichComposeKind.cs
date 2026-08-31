// What the composer is composing. Its own file, free of WinUI and of the localised ComposeRequest
// around it, so the rules that turn it into behaviour can be linked into Mailcal.Tests and pinned
// there, the signature slot in particular (ComposerSignatures.SlotFor), where "a reply, a reply-all
// and a forward share one slot" is a contract from docs/signatures.md rather than a local choice.

namespace Allodia.Mailcal.ViewModels;

/// <summary>What the rich composer is for.</summary>
public enum RichComposeKind
{
    /// <summary>A brand-new message (To/Cc/Bcc + Subject entered).</summary>
    New,

    /// <summary>A reply (To pre-filled with the sender; <c>Re:</c> subject + threading derived).</summary>
    Reply,

    /// <summary>A reply-all (To + Cc pre-filled with the thread participants; <c>Re:</c> derived).</summary>
    ReplyAll,

    /// <summary>A forward (recipients entered fresh; <c>Fwd:</c> subject derived).</summary>
    Forward,
}
