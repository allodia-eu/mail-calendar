//! One CSS colour, and where it moves to on a dark page.
//!
//! The move keeps the colour's hue and changes only its perceived lightness, in OKLab, so a
//! brand's blue stays blue and a red warning stays red. What it moves to depends on what the
//! colour is *for* ([`Role`]): a light fill becomes a dark one, dark ink becomes light, and a
//! rule or border swaps its lightness end for end. A fill or ink already right for a dark page is
//! left where it is, which is what lets a sender's own dark styling through untouched; a line has
//! no such colour, because what it carries is its contrast with the page.

use super::named::named_colour;

/// What a colour paints, which decides where it moves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    /// A fill text sits on: `background`, `bgcolor`.
    Fill,
    /// Text: `color`, `<font color>`.
    Ink,
    /// A line drawn against the page: a border, an outline, an underline.
    Line,
}

/// An sRGB colour with its alpha, each channel in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// OKLab lightness of the dark canvas, `#1e1e1e` (`super::super::document::DARK_CANVAS`).
/// White maps here, so a message's white page and the canvas around it are one colour.
const CANVAS_L: f32 = 0.2350;

/// OKLab lightness of the dark canvas's ink, `#e3e3e3`. Black text maps here.
const INK_L: f32 = 0.9158;

/// Where a mid-light fill lands: the lightest a fill gets on a dark page.
const FILL_CEILING: f32 = 0.42;

/// The darkest ink gets on a dark page: 4.5:1 against the canvas, and comfortably more against
/// any fill [`Role::Fill`] produces.
const INK_FLOOR: f32 = 0.72;

/// Below this a colour reads as dark, above it as light.
const MIDPOINT: f32 = 0.5;

/// The colour `role` should become on a dark page, or `None` when it is already right for one.
pub(super) fn darken(colour: Rgba, role: Role) -> Option<Rgba> {
    let lab = Lab::from_rgb(colour);
    let l = match role {
        Role::Fill if lab.l > MIDPOINT => {
            CANVAS_L + (1.0 - lab.l) / (1.0 - MIDPOINT) * (FILL_CEILING - CANVAS_L)
        }
        Role::Ink if lab.l < MIDPOINT => INK_L - lab.l / MIDPOINT * (INK_L - INK_FLOOR),
        Role::Ink if lab.l < INK_FLOOR => INK_FLOOR,
        Role::Line => CANVAS_L + (1.0 - lab.l) * (INK_L - CANVAS_L),
        Role::Fill | Role::Ink => return None,
    };
    let moved = Lab { l, ..lab }.to_rgb_in_gamut(colour.a);
    (moved.to_hex() != colour.to_hex()).then_some(moved)
}

impl Rgba {
    /// A `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa` body, without its `#`.
    pub(super) fn from_hex(hex: &str) -> Option<Self> {
        let digit = |i: usize, len: usize| -> Option<f32> {
            let s = hex.get(i * len..(i + 1) * len)?;
            let v = u8::from_str_radix(s, 16).ok()?;
            Some(
                if len == 1 {
                    f32::from(v * 17)
                } else {
                    f32::from(v)
                } / 255.0,
            )
        };
        let (len, alpha) = match hex.len() {
            3 => (1, false),
            4 => (1, true),
            6 => (2, false),
            8 => (2, true),
            _ => return None,
        };
        Some(Self {
            r: digit(0, len)?,
            g: digit(1, len)?,
            b: digit(2, len)?,
            a: if alpha { digit(3, len)? } else { 1.0 },
        })
    }

    /// A CSS named colour, case-insensitively. `transparent` and `currentcolor` are not colours
    /// a page can be darkened by, so neither is one here.
    pub(super) fn from_name(name: &str) -> Option<Self> {
        let [r, g, b] = named_colour(&name.to_ascii_lowercase())?;
        Some(Self::from_bytes(r, g, b, 1.0))
    }

    /// `rgb()`/`rgba()` arguments, legacy or modern syntax.
    pub(super) fn from_rgb_args(args: &[Component]) -> Option<Self> {
        let channel = |c: &Component| match *c {
            Component::Number(v) => Some((v / 255.0).clamp(0.0, 1.0)),
            Component::Percent(v) => Some(v.clamp(0.0, 1.0)),
            Component::Angle(_) => None,
        };
        let (rgb, a) = split_alpha(args)?;
        Some(Self {
            r: channel(&rgb[0])?,
            g: channel(&rgb[1])?,
            b: channel(&rgb[2])?,
            a,
        })
    }

    /// `hsl()`/`hsla()` arguments, legacy or modern syntax.
    #[allow(clippy::many_single_char_names)] // the names the conversion is written in everywhere
    pub(super) fn from_hsl_args(args: &[Component]) -> Option<Self> {
        let (hsl, a) = split_alpha(args)?;
        let hue = match hsl[0] {
            Component::Number(deg) | Component::Angle(deg) => deg,
            Component::Percent(_) => return None,
        };
        let fraction = |c: &Component| match *c {
            Component::Percent(v) => Some(v.clamp(0.0, 1.0)),
            Component::Number(v) => Some((v / 100.0).clamp(0.0, 1.0)),
            Component::Angle(_) => None,
        };
        let (s, l) = (fraction(&hsl[1])?, fraction(&hsl[2])?);
        let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let h = hue.rem_euclid(360.0) / 60.0;
        let x = chroma * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
        let (r, g, b) = match h {
            h if h < 1.0 => (chroma, x, 0.0),
            h if h < 2.0 => (x, chroma, 0.0),
            h if h < 3.0 => (0.0, chroma, x),
            h if h < 4.0 => (0.0, x, chroma),
            h if h < 5.0 => (x, 0.0, chroma),
            _ => (chroma, 0.0, x),
        };
        let m = l - chroma / 2.0;
        Some(Self {
            r: r + m,
            g: g + m,
            b: b + m,
            a,
        })
    }

    pub(super) fn from_bytes(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a,
        }
    }

    /// `#rrggbb` when opaque, `rgba(…)` otherwise: the forms every engine reads.
    pub(super) fn to_css(self) -> String {
        if self.a >= 1.0 {
            self.to_hex()
        } else {
            let [r, g, b] = self.bytes();
            let a = (self.a.clamp(0.0, 1.0) * 1000.0).round() / 1000.0;
            format!("rgba({r},{g},{b},{a})")
        }
    }

    fn to_hex(self) -> String {
        let [r, g, b] = self.bytes();
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to 0..=255 first
    fn bytes(self) -> [u8; 3] {
        [self.r, self.g, self.b].map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
    }
}

/// One argument of a colour function.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Component {
    Number(f32),
    /// Already divided by 100.
    Percent(f32),
    /// In degrees.
    Angle(f32),
}

/// Three channels and the alpha, which defaults to opaque.
fn split_alpha(args: &[Component]) -> Option<(&[Component], f32)> {
    match args.len() {
        3 => Some((args, 1.0)),
        4 => {
            let a = match args[3] {
                Component::Number(v) | Component::Percent(v) => v.clamp(0.0, 1.0),
                Component::Angle(_) => return None,
            };
            Some((&args[..3], a))
        }
        _ => None,
    }
}

/// A colour in OKLab: `l` is perceived lightness, `a` and `b` carry the hue and chroma.
#[derive(Debug, Clone, Copy)]
struct Lab {
    l: f32,
    a: f32,
    b: f32,
}

impl Lab {
    #[allow(clippy::many_single_char_names)] // the names the OKLab matrices are published in
    fn from_rgb(c: Rgba) -> Self {
        let (r, g, b) = (linear(c.r), linear(c.g), linear(c.b));
        let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_99 * b).cbrt();
        let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
        let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
        Self {
            l: 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
            a: 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
            b: 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
        }
    }

    /// Linear sRGB, unclamped: a channel outside `0..=1` means the colour is out of gamut.
    fn to_linear(self) -> [f32; 3] {
        let l = (self.l + 0.396_337_78 * self.a + 0.215_803_76 * self.b).powi(3);
        let m = (self.l - 0.105_561_346 * self.a - 0.063_854_17 * self.b).powi(3);
        let s = (self.l - 0.089_484_18 * self.a - 1.291_485_5 * self.b).powi(3);
        [
            4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
            -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
            -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
        ]
    }

    /// The nearest displayable colour of this lightness and hue: chroma is reduced until every
    /// channel fits, rather than clipping one channel and shifting the hue.
    fn to_rgb_in_gamut(self, alpha: f32) -> Rgba {
        let fits = |lab: Self| {
            lab.to_linear()
                .iter()
                .all(|c| (-1e-4..=1.0 + 1e-4).contains(c))
        };
        let mut lab = self;
        if !fits(lab) {
            let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
            for _ in 0..16 {
                let mid = f32::midpoint(lo, hi);
                if fits(Self {
                    a: self.a * mid,
                    b: self.b * mid,
                    ..self
                }) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            lab = Self {
                a: self.a * lo,
                b: self.b * lo,
                ..self
            };
        }
        let [r, g, b] = lab.to_linear().map(gamma);
        Rgba { r, g, b, a: alpha }
    }
}

fn linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn gamma(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
pub(super) fn lightness(c: Rgba) -> f32 {
    Lab::from_rgb(c).l
}

#[cfg(test)]
pub(super) fn hue(c: Rgba) -> f32 {
    let lab = Lab::from_rgb(c);
    lab.b.atan2(lab.a).to_degrees()
}

/// WCAG relative luminance, for the contrast assertions in the tests.
#[cfg(test)]
pub(super) fn luminance(c: Rgba) -> f32 {
    0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
}
