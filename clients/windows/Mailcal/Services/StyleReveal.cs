// What the reveal shows for one language of a learned style (docs/ai.md, "The reveal"), in the
// order every client draws it, with an empty field left out rather than drawn as a heading over
// nothing. WinUI-free and L10n-free, so Mailcal.Tests can pin the order and the skipping; the field
// names are WritingStyleText.

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using uniffi.mailcal_bindings;

namespace Allodia.Mailcal.Services;

/// <summary>One field of the description, in the order the reveal draws them.</summary>
internal enum RevealField
{
    /// <summary>How the person opens a message.</summary>
    Greetings,

    /// <summary>How they close one.</summary>
    SignOffs,

    /// <summary>The name they sign with.</summary>
    SignsAs,

    /// <summary>Register, and how it shifts.</summary>
    Register,

    /// <summary>Typical length in words.</summary>
    Length,

    /// <summary>Paragraphing and sentence length.</summary>
    Shape,

    /// <summary>Punctuation habits.</summary>
    Punctuation,

    /// <summary>How a reply is built.</summary>
    Structure,

    /// <summary>How they decline, chase, apologise and confirm.</summary>
    Moves,

    /// <summary>Characteristic phrases.</summary>
    Phrases,

    /// <summary>What they avoid.</summary>
    Avoid,
}

/// <summary>One field with something in it.</summary>
/// <param name="Field">Which field.</param>
/// <param name="Lines">What it says, one entry per line.</param>
/// <param name="Words">For <see cref="RevealField.Length"/>: the typical length in words.</param>
internal sealed record RevealSection(RevealField Field, IReadOnlyList<string> Lines, int Words = 0);

/// <summary>The reveal's content for one language.</summary>
internal static class StyleReveal
{
    /// <summary>The fields of <paramref name="language"/> that say something, in order.</summary>
    internal static IReadOnlyList<RevealSection> Sections(LanguageStyleRow language, CultureInfo culture)
    {
        var sections = new List<RevealSection>();
        void Add(RevealField field, IEnumerable<string> lines)
        {
            var kept = lines.Where(line => !string.IsNullOrWhiteSpace(line)).ToArray();
            if (kept.Length > 0)
            {
                sections.Add(new RevealSection(field, kept));
            }
        }
        Add(RevealField.Greetings, Habits(language.Greetings, culture));
        Add(RevealField.SignOffs, Habits(language.SignOffs, culture));
        Add(RevealField.SignsAs, new[] { language.SignsAs });
        Add(RevealField.Register, new[] { language.Register });
        if (language.TypicalWords > 0)
        {
            sections.Add(new RevealSection(
                RevealField.Length, Array.Empty<string>(), (int)language.TypicalWords));
        }
        Add(RevealField.Shape, new[] { language.Shape });
        Add(RevealField.Punctuation, new[] { language.Punctuation });
        Add(RevealField.Structure, new[] { language.Structure });
        Add(RevealField.Moves, new[] { language.Moves });
        Add(RevealField.Phrases, language.Phrases);
        Add(RevealField.Avoid, language.Avoid);
        return sections;
    }

    /// <summary>A habit as the reveal lists it: its exact wording, then roughly how often.</summary>
    internal static string Habit(HabitRow habit, CultureInfo culture) =>
        $"{habit.Text} ({WritingStyleFormat.Share(habit.Share, culture)})";

    private static IEnumerable<string> Habits(IEnumerable<HabitRow> habits, CultureInfo culture) =>
        habits.Where(habit => !string.IsNullOrWhiteSpace(habit.Text)).Select(habit => Habit(habit, culture));
}
