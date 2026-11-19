// Settings → Writing style as the core draws it (docs/ai.md rules 8 to 16): the page, the learning
// flow, a style's reveal and detail, and the changes they dispatch. Nothing is cached here: each is
// read from the core's memory when it is drawn.

using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

public sealed partial class MailboxModel
{
    /// <summary>Raised when Settings → Writing style may have changed (<c>Surface.WritingStyle</c>,
    /// and <c>Surface.Settings</c>): the page, the flow, the reveal and the detail read again.</summary>
    internal event Action? WritingStyleChanged;

    /// <summary>The page, or <c>null</c> while the writing assistant is not offered, when the
    /// category is not drawn at all.</summary>
    internal WritingStylePage? WritingStylePage() => _app?.WritingStylePage();

    /// <summary>The learning flow while a host draws it, or <c>null</c>.</summary>
    internal WritingStyleLearning? WritingStyleLearning() => _app?.WritingStyleLearning();

    /// <summary>The reveal of <paramref name="style"/>, or <c>null</c> when it names none.</summary>
    internal WritingStyleReveal? WritingStyleReveal(string style) => _app?.WritingStyleReveal(style);

    /// <summary>The detail of <paramref name="style"/>, or <c>null</c> when it names none.</summary>
    internal WritingStyleDetail? WritingStyleDetail(string style) => _app?.WritingStyleDetail(style);

    /// <summary>A change on the page, in the flow, the reveal or the detail. Returns at once.</summary>
    internal void ChangeWritingStyle(WritingStyleIntent intent) =>
        _app?.Dispatch(new Intent.Ai(new AiIntent.WritingStyle(intent)));

    /// <summary>A move of the learning flow. Returns at once.</summary>
    internal void MoveLearning(StyleLearningIntent intent) =>
        ChangeWritingStyle(new WritingStyleIntent.Learning(intent));

    /// <summary>Why a style named <paramref name="name"/> would not be saved, or <c>null</c>: the rule
    /// the reveal's and the detail's Save are held by, and the change refuses by.</summary>
    internal static WritingStyleRefusal? StyleNameRefusal(string name) =>
        MailcalBindingsMethods.WritingStyleNameRefusal(name);

    private void RaiseWritingStyleChanged() => WritingStyleChanged?.Invoke();
}
