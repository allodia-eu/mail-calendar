//! The shared colour palette, and resolving a colour into a legible swatch.
//!
//! Two surfaces draw from this: a calendar's colour, and a sender's avatar. They share it
//! on purpose: the same ten hues in both places read as one system rather than two
//! unrelated colour schemes, and the contrast guarantee below is decided once instead of
//! being re-derived per surface.
//!
//! A calendar's colour is *data*: the user's own coding, often carried over from years of
//! Google or Samsung use: not app chrome. So a server-supplied colour is honoured, but it
//! is snapped to the nearest entry of an Allodia palette, which keeps ten arbitrary hues
//! from a provider looking like ten arbitrary hues. The user can override any calendar.
//!
//! **Allodia Orange (`#F6A24A`) is not in the palette.** The brand reserves it for actions
//! (a "New event" button, a save) and a calendar the user happens to colour orange would
//! sit next to those looking clickable. The palette skips the orange band entirely and
//! spaces its hues around the rest of the wheel.
//!
//! # Contrast is resolved here, not per client
//!
//! Each colour resolves to a `(background, text, border)` triple for light **and** dark, so
//! three clients cannot disagree about whether a chip's label is readable. The text is
//! whichever of black or white contrasts better with the background, which is not a
//! heuristic: the worst-case background (the one equidistant from both) still clears
//! **4.58:1** against the better choice, comfortably over the WCAG AA threshold of 4.5:1
//! for normal text. So the choice is *always* legible, and a property test holds it there.

/// The Allodia calendar palette: ten hues, spaced around the wheel, at a consistent
/// lightness so no calendar shouts louder than another (the categorical-palette rule;
/// equal visual weight, since no calendar outranks the rest).
///
/// The orange band (roughly 20°–45°) is deliberately empty: that is Allodia Orange's, and
/// it means "action".
pub const PALETTE: [&str; 10] = [
    "#2f6fa8", // blue: the default, kin to Allodia Blue
    "#4f5ba6", // indigo
    "#7a4fa6", // violet
    "#a64f8e", // magenta
    "#a64f63", // rose
    "#a85046", // red
    "#7c8b2e", // olive
    "#3f8f55", // green
    "#2c8c82", // teal
    "#2183a0", // cyan
];

/// One theme's rendering of a calendar colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Swatch {
    /// The chip's fill, `#rrggbb`.
    pub background: String,
    /// The label colour on that fill, `#rrggbb`; always at least 4.5:1 against it.
    pub text: String,
    /// The chip's edge, `#rrggbb`.
    pub border: String,
}

/// A calendar colour, resolved for both themes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarColor {
    /// The palette colour it snapped to, `#rrggbb`; what a colour picker shows as selected.
    pub hex: String,
    /// How to draw it in a light theme.
    pub light: Swatch,
    /// How to draw it in a dark theme.
    pub dark: Swatch,
}

/// Resolves a calendar's color: an override if the user set one, else the server's color,
/// else a stable palette pick from `fallback_index`.
///
/// Any input (a `#hex`, a CSS colour name, or junk) snaps to the nearest palette entry, so
/// the grid never renders a colour the palette does not contain. `fallback_index` is
/// typically the calendar's position in the account, so a server that sends no colour still
/// gives its calendars distinct, stable hues rather than ten identical blue blocks.
#[must_use]
pub fn resolve(
    override_hex: Option<&str>,
    server: Option<&str>,
    fallback_index: usize,
) -> CalendarColor {
    let chosen = override_hex
        .and_then(parse)
        .or_else(|| server.and_then(parse))
        .map_or_else(|| palette_at(fallback_index), nearest_in_palette);
    swatches(chosen)
}

/// The palette entry at `index`, wrapping: so a calendar always gets *a* colour.
fn palette_at(index: usize) -> Rgb {
    parse(PALETTE[index % PALETTE.len()]).expect("the palette is valid hex")
}

/// Builds both themes' swatches from a palette colour.
fn swatches(color: Rgb) -> CalendarColor {
    // A calendar chip reads best as saturated colour under a white label: the look every
    // mainstream calendar uses. A raw mid-tone palette hue (the green, the olive) is a shade
    // too light for that: white just misses AA on it, so `readable_on` falls back to a *dark*
    // label: the muddy black-on-green people report. So sink the light fill a fifth of the
    // way to black: deep enough that white wins cleanly on every palette hue, while the `hex`
    // field keeps the bright, recognisable colour a picker shows as selected. Arbitrary user
    // overrides still get whichever of black/white `readable_on` proves legible.
    let light_bg = mix(color, Rgb::BLACK, 20);
    // On a dark surface a fully-saturated chip glows. Sinking it a third of the way to black
    // keeps the hue recognisable while letting the grid's lines stay visible through the
    // layout, and keeps it visibly darker than the light fill, so the themes don't converge.
    let dark_bg = mix(color, Rgb::BLACK, 34);
    CalendarColor {
        hex: color.to_hex(),
        light: Swatch {
            background: light_bg.to_hex(),
            text: readable_on(light_bg).to_hex(),
            border: mix(light_bg, Rgb::BLACK, 20).to_hex(),
        },
        dark: Swatch {
            background: dark_bg.to_hex(),
            text: readable_on(dark_bg).to_hex(),
            border: mix(dark_bg, Rgb::WHITE, 20).to_hex(),
        },
    }
}

/// Black or white, whichever contrasts more with `background`.
///
/// Never a near-black or near-white: the worst-case background clears 4.58:1 against pure
/// black or white, but only ~3.8:1 against a `#1a1a1a`, which fails AA outright. The extra
/// "softness" of an off-black would cost legibility on exactly the colours that need it.
fn readable_on(background: Rgb) -> Rgb {
    if contrast(background, Rgb::WHITE) >= contrast(background, Rgb::BLACK) {
        Rgb::WHITE
    } else {
        Rgb::BLACK
    }
}

/// The WCAG 2.x contrast ratio between two colours, `1.0..=21.0`.
#[must_use]
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// The index of the palette entry closest to `color`.
///
/// Distance is "redmean": a cheap approximation of perceptual distance that is markedly
/// better than plain RGB Euclidean at the reds and blues, where a naive metric happily
/// snaps a crimson to a navy.
fn nearest_palette_index(color: Rgb) -> usize {
    PALETTE
        .iter()
        .enumerate()
        .filter_map(|(index, hex)| parse(hex).map(|rgb| (index, rgb)))
        .min_by(|(_, a), (_, b)| {
            distance(color, *a)
                .partial_cmp(&distance(color, *b))
                .unwrap_or(core::cmp::Ordering::Equal)
        })
        .map_or(0, |(index, _)| index)
}

/// The palette entry closest to `color`.
fn nearest_in_palette(color: Rgb) -> Rgb {
    palette_at(nearest_palette_index(color))
}

/// The palette slot a colour snaps to; how a caller learns which hues are already spoken for.
///
/// Used to hand a calendar that has no colour of its own a hue **no other calendar is already
/// using** (see the calendar cache): a `#hex`, a CSS name, or junk that parses returns its nearest
/// palette slot; input that does not parse returns `None`; it lays no claim on a hue.
#[must_use]
pub fn nearest_index(hex: &str) -> Option<usize> {
    parse(hex).map(nearest_palette_index)
}

/// The redmean colour distance (squared) between two colours.
fn distance(a: Rgb, b: Rgb) -> f64 {
    let red_mean = f64::midpoint(f64::from(a.r), f64::from(b.r));
    let (dr, dg, db) = (
        f64::from(a.r) - f64::from(b.r),
        f64::from(a.g) - f64::from(b.g),
        f64::from(a.b) - f64::from(b.b),
    );
    (2.0 + red_mean / 256.0) * dr * dr
        + 4.0 * dg * dg
        + (2.0 + (255.0 - red_mean) / 256.0) * db * db
}

/// Blends `percent` of `into` on top of `color`.
///
/// Integer arithmetic, so there is no float-to-`u8` cast to truncate or lose a sign: a
/// convex combination of two `u8`s cannot leave `0..=255`, and the rounding is explicit.
fn mix(color: Rgb, into: Rgb, percent: u32) -> Rgb {
    let percent = percent.min(100);
    let blend = |a: u8, b: u8| {
        let scaled = u32::from(a) * (100 - percent) + u32::from(b) * percent;
        u8::try_from(scaled.div_ceil(100).min(255)).unwrap_or(u8::MAX)
    };
    Rgb {
        r: blend(color.r, into.r),
        g: blend(color.g, into.g),
        b: blend(color.b, into.b),
    }
}

/// An 8-bit sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

impl Rgb {
    /// Black.
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    /// White.
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    /// The `#rrggbb` form.
    #[must_use]
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// The WCAG relative luminance, `0.0..=1.0`.
    #[must_use]
    pub fn luminance(self) -> f64 {
        let channel = |value: u8| {
            let v = f64::from(value) / 255.0;
            if v <= 0.039_28 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126f64.mul_add(
            channel(self.r),
            0.7152f64.mul_add(channel(self.g), 0.0722 * channel(self.b)),
        )
    }
}

/// Parses a colour a provider might send: `#rgb`, `#rrggbb`, or one of the CSS names
/// CalDAV and JMAP servers actually emit. Anything else is `None`, and the caller falls
/// back to the palette rather than rendering a guess.
fn parse(raw: &str) -> Option<Rgb> {
    let raw = raw.trim();
    if let Some(hex) = raw.strip_prefix('#') {
        return match hex.len() {
            // `#abc` is shorthand for `#aabbcc`.
            3 => {
                let digit = |i: usize| u8::from_str_radix(&hex[i..=i], 16).ok();
                let (r, g, b) = (digit(0)?, digit(1)?, digit(2)?);
                Some(Rgb {
                    r: r * 17,
                    g: g * 17,
                    b: b * 17,
                })
            }
            6 => Some(Rgb {
                r: u8::from_str_radix(&hex[0..2], 16).ok()?,
                g: u8::from_str_radix(&hex[2..4], 16).ok()?,
                b: u8::from_str_radix(&hex[4..6], 16).ok()?,
            }),
            _ => None,
        };
    }
    named(&raw.to_ascii_lowercase())
}

/// The CSS colour names real servers send. Not the full CSS list: a calendar colour picked
/// in Apple Calendar, Thunderbird, or a Nextcloud instance lands in this handful.
fn named(name: &str) -> Option<Rgb> {
    let hex = match name {
        "red" => "#ff0000",
        "green" => "#008000",
        "blue" => "#0000ff",
        "yellow" => "#ffff00",
        "orange" => "#ffa500",
        "purple" => "#800080",
        "magenta" | "fuchsia" => "#ff00ff",
        "cyan" | "aqua" => "#00ffff",
        "teal" => "#008080",
        "olive" => "#808000",
        "navy" => "#000080",
        "maroon" => "#800000",
        "lime" => "#00ff00",
        "pink" => "#ffc0cb",
        "brown" => "#a52a2a",
        "gray" | "grey" => "#808080",
        "silver" => "#c0c0c0",
        "black" => "#000000",
        "white" => "#ffffff",
        _ => return None,
    };
    parse(hex)
}

#[cfg(test)]
#[path = "color_tests.rs"]
mod color_tests;
