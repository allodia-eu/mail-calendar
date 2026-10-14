//! Finding the colours in a message's CSS, and the `prefers-color-scheme` queries that would let
//! the web view rather than the reader decide which of the sender's styles apply.
//!
//! The walk records edits as byte ranges of the text it was given and replaces nothing else, so
//! whatever it does not recognise (a hack, a typo, a property it has no role for) reaches the page
//! exactly as the sender wrote it. CSS that fails to tokenise sensibly is not an error here: it is
//! left as it was.

use cssparser::{Parser, Token};

use super::{
    MessageScheme,
    colour::{Component, Rgba, Role, darken},
};

/// One replacement: the bytes `start..end` of the input become `text`.
pub(super) struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// The edits that adapt a `<style>` sheet to `scheme`.
pub(super) fn sheet_edits(css: &str, scheme: MessageScheme) -> Vec<Edit> {
    let mut parser = Parser::new(css);
    let mut edits = Vec::new();
    rules(&mut parser, scheme, true, &mut edits);
    edits
}

/// The edits that adapt a `style` attribute's declarations to a dark page.
pub(super) fn declaration_edits(css: &str) -> Vec<Edit> {
    let mut parser = Parser::new(css);
    let mut edits = Vec::new();
    declarations(&mut parser, &mut edits);
    edits
}

/// What the tokens before the next `{` introduce.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Prelude {
    /// A selector: its block holds declarations.
    Style,
    /// `@media`: its block holds rules, and its query may ask for a colour scheme.
    Media,
    /// `@supports`, `@keyframes` and the like: a block of rules.
    GroupAt,
    /// `@font-face`, `@page`: a block of declarations that paints nothing.
    DescriptorAt,
}

/// A list of rules: a sheet, or the body of an `@media` or `@supports`. `colours` is false inside
/// a sender's own `prefers-color-scheme: dark` block on a dark page: those colours were chosen for
/// a dark page already, so they reach it as written.
fn rules(p: &mut Parser, scheme: MessageScheme, colours: bool, edits: &mut Vec<Edit>) {
    let mut prelude = Prelude::Style;
    let mut senders_dark = false;
    loop {
        let start = p.position().byte_index();
        let Ok(token) = p.next_including_whitespace_and_comments() else {
            return;
        };
        match token {
            Token::AtKeyword(name) => {
                senders_dark = false;
                prelude = match name.to_ascii_lowercase().as_str() {
                    "media" => Prelude::Media,
                    "font-face" | "page" | "counter-style" | "property" => Prelude::DescriptorAt,
                    _ => Prelude::GroupAt,
                };
            }
            Token::Semicolon => {
                prelude = Prelude::Style;
                senders_dark = false;
            }
            Token::ParenthesisBlock if prelude == Prelude::Media => {
                let feature = p
                    .parse_nested_block(|p| Ok::<_, cssparser::ParseError<()>>(scheme_feature(p)))
                    .ok()
                    .flatten();
                if let Some(asks_for_dark) = feature {
                    let matches = asks_for_dark == (scheme == MessageScheme::Dark);
                    senders_dark |= asks_for_dark && matches;
                    edits.push(Edit {
                        start,
                        end: p.position().byte_index(),
                        text: if matches { ALWAYS } else { NEVER }.to_owned(),
                    });
                }
            }
            Token::CurlyBracketBlock => {
                let block = std::mem::replace(&mut prelude, Prelude::Style);
                let inner_colours = colours && !std::mem::take(&mut senders_dark);
                // A block the walk does not enter is skipped whole by the next token read.
                let _ = p.parse_nested_block(|p| {
                    match block {
                        Prelude::Media | Prelude::GroupAt => rules(p, scheme, inner_colours, edits),
                        Prelude::Style if scheme == MessageScheme::Dark && colours => {
                            declarations(p, edits);
                        }
                        Prelude::Style | Prelude::DescriptorAt => {
                            while p.next_including_whitespace_and_comments().is_ok() {}
                        }
                    }
                    Ok::<_, cssparser::ParseError<()>>(())
                });
            }
            _ => {}
        }
    }
}

/// A media feature that is always true and one that is never: the width of a laid-out page is
/// never negative. Replacing only the feature keeps the rest of the query (`screen and …`) as the
/// sender wrote it.
const ALWAYS: &str = "(min-width:0px)";
const NEVER: &str = "(max-width:-1px)";

/// `Some(true)` for `prefers-color-scheme: dark`, `Some(false)` for `: light`, else `None`.
fn scheme_feature(p: &mut Parser) -> Option<bool> {
    let mut words = Vec::new();
    while let Ok(token) = p.next() {
        match token {
            Token::Ident(word) => words.push(word.to_ascii_lowercase()),
            Token::Colon => {}
            _ => return None,
        }
    }
    match words.as_slice() {
        [feature, value] if feature == "prefers-color-scheme" => match value.as_str() {
            "dark" => Some(true),
            "light" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Where a declaration list is in its grammar.
enum State {
    Name,
    Colon(Option<Role>, bool),
    Value(Option<Role>, bool),
    /// Junk until the next `;`.
    Skip,
}

/// A declaration list. When any of its backgrounds is an image, the whole block keeps its
/// colours: text over a picture was chosen for the picture, which stays as it is.
fn declarations(p: &mut Parser, edits: &mut Vec<Edit>) {
    let mut block = Vec::new();
    let mut has_image = false;
    let mut state = State::Name;
    loop {
        let start = p.position().byte_index();
        let Ok(token) = p.next_including_whitespace_and_comments() else {
            break;
        };
        state = match (state, token) {
            (_, Token::Semicolon) => State::Name,
            (state, Token::WhiteSpace(_) | Token::Comment(_)) => state,
            (State::Name, Token::Ident(name)) => {
                let name = name.to_ascii_lowercase();
                State::Colon(role_of(&name), name.starts_with("background"))
            }
            (State::Colon(role, background), Token::Colon) => State::Value(role, background),
            (State::Value(role, background), token) => {
                let token = token.clone();
                if let Some(role) = role {
                    value_token(
                        p,
                        &token,
                        start,
                        role,
                        background,
                        &mut block,
                        &mut has_image,
                    );
                }
                State::Value(role, background)
            }
            _ => State::Skip,
        };
    }
    if !has_image {
        edits.append(&mut block);
    }
}

/// The colour role a property paints in, or `None` for one that is left alone (a shadow, a fill
/// inside an SVG, anything not a colour).
fn role_of(property: &str) -> Option<Role> {
    let property = property.trim_start_matches("-webkit-");
    if property.starts_with("background") {
        Some(Role::Fill)
    } else if matches!(property, "color" | "text-fill-color" | "caret-color") {
        Some(Role::Ink)
    } else if ["border", "outline", "text-decoration", "column-rule"]
        .iter()
        .any(|prefix| property.starts_with(prefix))
    {
        Some(Role::Line)
    } else {
        None
    }
}

fn is_image(token: &Token) -> bool {
    match token {
        Token::UnquotedUrl(_) => true,
        Token::Function(name) => {
            let name = name.to_ascii_lowercase();
            matches!(name.trim_start_matches("-webkit-"), "url" | "image-set")
        }
        _ => false,
    }
}

/// One token of a value: a colour becomes an edit, and a function that is not a colour (a
/// gradient) is searched for the colours inside it. An image found inside one counts only in a
/// `background`: a `border-image` frames the block rather than painting behind its text.
fn value_token(
    p: &mut Parser,
    token: &Token,
    start: usize,
    role: Role,
    background: bool,
    edits: &mut Vec<Edit>,
    has_image: &mut bool,
) {
    *has_image |= background && is_image(token);
    let colour = match token {
        Token::Hash(hex) | Token::IDHash(hex) => Rgba::from_hex(hex),
        Token::Ident(name) => Rgba::from_name(name),
        Token::Function(name) => {
            let name = name.to_ascii_lowercase();
            if matches!(name.as_str(), "rgb" | "rgba" | "hsl" | "hsla") {
                let args = p
                    .parse_nested_block(|p| Ok::<_, cssparser::ParseError<()>>(components(p)))
                    .ok()
                    .flatten();
                args.and_then(|args| {
                    if name.starts_with("rgb") {
                        Rgba::from_rgb_args(&args)
                    } else {
                        Rgba::from_hsl_args(&args)
                    }
                })
            } else {
                let _ = p.parse_nested_block(|p| {
                    loop {
                        let start = p.position().byte_index();
                        let Ok(inner) = p.next_including_whitespace_and_comments() else {
                            break;
                        };
                        let inner = inner.clone();
                        value_token(p, &inner, start, role, background, edits, has_image);
                    }
                    Ok::<_, cssparser::ParseError<()>>(())
                });
                None
            }
        }
        _ => None,
    };
    if let Some(moved) = colour.and_then(|c| darken(c, role)) {
        edits.push(Edit {
            start,
            end: p.position().byte_index(),
            text: moved.to_css(),
        });
    }
}

/// The numbers inside `rgb(…)` or `hsl(…)`, in either syntax: commas, or spaces and a `/`.
fn components(p: &mut Parser) -> Option<Vec<Component>> {
    let mut out = Vec::new();
    while let Ok(token) = p.next() {
        out.push(match *token {
            Token::Number { value, .. } => Component::Number(value),
            Token::Percentage { unit_value, .. } => Component::Percent(unit_value),
            Token::Dimension {
                value, ref unit, ..
            } => Component::Angle(degrees(value, unit)?),
            Token::Comma | Token::Delim('/') => continue,
            _ => return None,
        });
    }
    Some(out)
}

fn degrees(value: f32, unit: &str) -> Option<f32> {
    match unit.to_ascii_lowercase().as_str() {
        "deg" => Some(value),
        "rad" => Some(value.to_degrees()),
        "grad" => Some(value * 0.9),
        "turn" => Some(value * 360.0),
        _ => None,
    }
}
