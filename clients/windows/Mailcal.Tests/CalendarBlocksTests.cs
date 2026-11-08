// An event block's label at a zoom (docs/calendar.md §4): which lines it draws, where a long title
// wraps, when the corner glyphs give way and when the time is a range. The same numbers as Apple's
// CalendarBlocks.swift and Linux's blocks.rs, so the three grids agree on what a block shows.
using Allodia.Mailcal.Calendar;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class CalendarBlocksTests
{
    [Fact]
    public void A_block_too_short_for_a_whole_line_draws_no_title()
    {
        // A quarter of an hour at twenty pixels an hour is five pixels tall.
        Assert.False(CalendarBlocks.ShowsLabel(15, 20f));
        Assert.Equal(0, CalendarBlocks.TitleLines(15, 20f));
        // The same quarter at sixty an hour has room for one line, and no time beneath it.
        Assert.True(CalendarBlocks.ShowsLabel(15, 60f));
        Assert.False(CalendarBlocks.ShowsTime(15, 60f));
        Assert.Equal(1, CalendarBlocks.TitleLines(15, 60f));
    }

    [Fact]
    public void A_long_block_wraps_its_title_over_every_whole_line_less_the_time()
    {
        // Three hours at sixty an hour: 176 px of room over a 14 px pitch is 12 whole lines, one of
        // which the time takes.
        Assert.True(CalendarBlocks.ShowsTime(180, 60f));
        Assert.Equal(11, CalendarBlocks.TitleLines(180, 60f));
        // An hour: 56 px is four lines, three for the title.
        Assert.Equal(3, CalendarBlocks.TitleLines(60, 60f));
        // Half an hour at sixty: 26 px holds one line, so no time and one title line.
        Assert.False(CalendarBlocks.ShowsTime(30, 60f));
        Assert.Equal(1, CalendarBlocks.TitleLines(30, 60f));
    }

    [Fact]
    public void A_title_never_breaks_a_word()
    {
        Assert.True(CalendarBlocks.Wraps(40f, 40f));
        Assert.False(CalendarBlocks.Wraps(41f, 40f));
    }

    [Fact]
    public void The_wrapping_width_is_held_on_a_step_so_a_resize_reuses_a_layout()
    {
        Assert.Equal(48f, CalendarBlocks.WrapWidth(51.9f));
        Assert.Equal(48f, CalendarBlocks.WrapWidth(48f));
        Assert.Equal(0f, CalendarBlocks.WrapWidth(-3f));
        Assert.Equal(CalendarBlocks.WrapWidth(49f), CalendarBlocks.WrapWidth(51f));
        Assert.NotEqual(CalendarBlocks.WrapWidth(51f), CalendarBlocks.WrapWidth(52f));
    }

    [Fact]
    public void The_corner_glyphs_draw_only_while_the_title_keeps_thirty_six_pixels()
    {
        var two = CalendarBlocks.GlyphsWidth(2, 11f);
        Assert.Equal(25f, two);
        Assert.Equal(36f, CalendarBlocks.BesideGlyphs(64f, two));
        Assert.Null(CalendarBlocks.BesideGlyphs(63f, two));
        Assert.Null(CalendarBlocks.BesideGlyphs(200f, CalendarBlocks.GlyphsWidth(0, 11f)));
    }

    [Fact]
    public void The_time_is_a_range_where_it_fits_and_the_start_where_it_does_not()
    {
        Assert.True(CalendarBlocks.ShowsRange(60f, 60f));
        Assert.False(CalendarBlocks.ShowsRange(61f, 60f));
    }

    [Fact]
    public void The_last_title_line_drawn_is_whole_and_leaves_the_time_its_room()
    {
        // Lines drawn 15 px apart, a little taller than the 14 px pitch they were counted in.
        float[] bottoms = [15f, 30f, 45f, 60f];
        // Three lines counted, but 44 px of room keeps only the two that end inside it.
        Assert.Equal(30f, CalendarBlocks.Fitting(bottoms, 3, 44f));
        // Room for all three.
        Assert.Equal(45f, CalendarBlocks.Fitting(bottoms, 3, 45f));
        // Never more than were counted, however much room there is.
        Assert.Equal(30f, CalendarBlocks.Fitting(bottoms, 2, 500f));
        // Never fewer than one: a block with a label always draws its first line.
        Assert.Equal(15f, CalendarBlocks.Fitting(bottoms, 3, 10f));
        // A title shorter than the room has no more lines than it has.
        Assert.Equal(15f, CalendarBlocks.Fitting([15f], 5, 100f));
    }
}
