//! A message adapted to the scheme it is read in: which of its colours move, which stay, and that
//! nothing but a colour or a colour-scheme query is ever changed.

use std::borrow::Cow;

use super::{MessageScheme, adapt};
use crate::html::sanitize;

/// `raw` as the reading view gets it: through the real sanitiser, so the walk is tested against
/// the serialisation it actually reads.
fn dark(raw: &str) -> String {
    adapt(&sanitize(raw).html, MessageScheme::Dark).into_owned()
}

fn light(raw: &str) -> String {
    adapt(&sanitize(raw).html, MessageScheme::Light).into_owned()
}

#[test]
fn a_light_render_of_an_ordinary_message_is_the_fragment_itself() {
    // Most mail has no colour-scheme query, and a light read of it is exactly what it was before
    // dark rendering existed: not a copy, not a re-serialisation.
    let fragment = sanitize("<p style=\"color:#333\">hi</p>").html;
    assert!(matches!(
        adapt(&fragment, MessageScheme::Light),
        Cow::Borrowed(_)
    ));
}

#[test]
fn a_dark_render_moves_inline_colours_and_keeps_the_rest_of_the_declaration() {
    let out = dark(
        r#"<p style="color:#000000 !important;font-family:'Google Sans', Arial;background-color:#FFFFFF">x</p>"#,
    );
    assert!(out.contains("color:#e3e3e3 !important"), "{out}");
    assert!(out.contains("background-color:#1e1e1e"), "{out}");
    assert!(out.contains("font-family:'Google Sans', Arial"), "{out}");
}

#[test]
fn an_escaped_quote_in_a_style_attribute_survives_the_move() {
    // The serialiser writes `"` inside an attribute as `&quot;`; the walk reads the CSS decoded and
    // writes it back escaped, so the attribute still ends where it did.
    let out = dark(r#"<p style='font-family:"Segoe UI";color:black'>x</p>"#);
    assert!(
        out.contains(r#"style="font-family:&quot;Segoe UI&quot;;color:#e3e3e3""#),
        "{out}"
    );
}

#[test]
fn legacy_colour_attributes_move_too() {
    // Table layouts colour their cells with `bgcolor`, and Thunderbird still writes `<font color>`.
    let out = dark(
        r##"<table bgcolor="#ffffff"><tr><td bgcolor="F8F9FA"><font color="black">x</font></td></tr></table>"##,
    );
    assert!(out.contains("bgcolor=\"#1e1e1e\""), "{out}");
    assert!(!out.contains("F8F9FA"), "{out}");
    assert!(out.contains("color=\"#e3e3e3\""), "{out}");
}

#[test]
fn a_style_sheet_moves_its_colours_and_leaves_its_selectors_alone() {
    let out = dark(
        "<style>a:hover{color:#000}.card{background:#fff;border:1px solid #ddd}\
         @media only screen and (max-width:600px){.x{color:rgb(0,0,0)}}</style><p>x</p>",
    );
    assert!(out.contains("a:hover{color:#e3e3e3}"), "{out}");
    assert!(
        out.contains(".card{background:#1e1e1e;border:1px solid #"),
        "{out}"
    );
    assert!(
        out.contains("(max-width:600px){.x{color:#e3e3e3}}"),
        "{out}"
    );
}

#[test]
fn every_colour_function_is_recognised() {
    let out = dark(
        "<p style=\"color:rgba(0, 0, 0, 0.87)\">a</p><p style=\"color:hsl(0 0% 0%)\">b</p>\
         <p style=\"background:linear-gradient(#ffffff, rgb(248 249 250))\">c</p>",
    );
    assert!(out.contains("color:rgba(227,227,227,0.87)"), "{out}");
    assert!(out.contains("color:#e3e3e3\">b"), "{out}");
    assert!(out.contains("linear-gradient(#1e1e1e, #"), "{out}");
    assert!(!out.contains("rgb(248 249 250)"), "{out}");
}

#[test]
fn a_block_painted_over_an_image_keeps_its_colours() {
    // Text over a picture was chosen for the picture, and the picture is not darkened; moving the
    // text alone would put light ink on a light photograph.
    let raw = r#"<td style="background:#ffffff url('https://example.com/hero.png');color:#000000">x</td>"#;
    assert_eq!(dark(raw), light(raw));
}

#[test]
fn only_a_background_image_keeps_a_block_s_colours() {
    // A framing picture is not behind the text, so the block's own page and ink still move.
    let framed = ".card{border-image:url(frame.png) 30;background:#ffffff;color:#000000}";
    let framed = format!("<style>{framed}</style>");
    let moved = adapt(&framed, MessageScheme::Dark);
    assert!(
        !moved.contains("#ffffff") && !moved.contains("#000000"),
        "{moved}"
    );
    // An image inside a function is still one when it is the background.
    let hero = ".hero{background:-webkit-image-set(url(hero.png) 1x);color:#000000}";
    let hero = format!("<style>{hero}</style>");
    let kept = adapt(&hero, MessageScheme::Dark);
    assert!(kept.contains("color:#000000"), "{kept}");
}

#[test]
fn a_colour_already_right_for_a_dark_page_is_not_rewritten() {
    let raw = r#"<div style="background:#202124;color:#ffffff">x</div>"#;
    assert_eq!(dark(raw), light(raw));
}

#[test]
fn the_senders_own_colour_scheme_queries_follow_the_page_being_drawn() {
    // Which of a sender's `prefers-color-scheme` rules apply is decided here, never by the web
    // view, whose idea of the system's scheme differs per platform and need not match the app's.
    let raw = "<style>@media (prefers-color-scheme: dark){.logo-dark{display:block}}\
               @media screen and (prefers-color-scheme:light){.logo-light{display:block}}</style>";
    let dark = dark(raw);
    assert!(dark.contains("@media (min-width:0px){.logo-dark"), "{dark}");
    assert!(
        dark.contains("screen and (max-width:-1px){.logo-light"),
        "{dark}"
    );
    let light = light(raw);
    assert!(
        light.contains("@media (max-width:-1px){.logo-dark"),
        "{light}"
    );
    assert!(
        light.contains("screen and (min-width:0px){.logo-light"),
        "{light}"
    );
}

#[test]
fn a_light_render_moves_no_colour() {
    let raw = "<style>@media (prefers-color-scheme: dark){.x{color:#fff}}.y{color:#000}</style>\
               <p style=\"color:#000\" bgcolor=\"#fff\">x</p>";
    let out = light(raw);
    assert!(out.contains(".y{color:#000}"), "{out}");
    assert!(out.contains("style=\"color:#000\""), "{out}");
}

#[test]
fn css_the_walk_does_not_understand_reaches_the_page_as_written() {
    // Malformed CSS is ordinary in mail. Nothing here may panic, and what is not a colour the walk
    // recognises is left exactly as the sender wrote it.
    for raw in [
        "<p style=\"color:\">x</p>",
        "<p style=\"color:#12\">x</p>",
        "<p style=\"*zoom:1;_color:red;color red\">x</p>",
        "<p style=\"color:rgb(1,2)\">x</p>",
        "<p style=\"color:var(--brand)\">x</p>",
        "<style>}}{{color:#000</style>",
        "<style>@media (prefers-color-scheme){x{y:z}}</style>",
        "<p style=\"font-family:'日本語';color:inherit\">ü</p>",
    ] {
        let sanitised = sanitize(raw).html;
        let out = adapt(&sanitised, MessageScheme::Dark);
        assert_eq!(
            out.matches('<').count(),
            sanitised.matches('<').count(),
            "{raw}: {out}"
        );
    }
    assert_eq!(
        dark("<p style=\"color:var(--brand)\">x</p>"),
        light("<p style=\"color:var(--brand)\">x</p>")
    );
}

#[test]
fn a_dark_render_adds_no_markup() {
    // The walk runs after the sanitiser, so what it writes must be a colour and nothing else: the
    // same elements, in the same places, with no new character that could open a tag.
    let raw = r##"<style>.a{color:#000}</style><table bgcolor="#fff" style="color:black">
        <tr><td><font color="#333">x &lt; y</font><a href="https://example.com/?a=1&amp;b=2">l</a></td></tr></table>"##;
    let (before, after) = (sanitize(raw).html, dark(raw));
    assert_ne!(before, after);
    for ch in ['<', '>', '"'] {
        assert_eq!(
            before.matches(ch).count(),
            after.matches(ch).count(),
            "{ch}: {after}"
        );
    }
    assert!(
        after.contains("https://example.com/?a=1&amp;b=2"),
        "{after}"
    );
}

#[test]
fn the_senders_own_dark_rules_reach_a_dark_page_as_written() {
    let raw = "<style>.card{border:1px solid #444}\
               @media (prefers-color-scheme: dark){.card{border-color:#444;background:#fafafa}}</style>";
    let dark = dark(raw);
    assert!(
        dark.contains("{.card{border-color:#444;background:#fafafa}}"),
        "{dark}"
    );
    assert!(
        !dark.starts_with("<style>.card{border:1px solid #444}"),
        "{dark}"
    );
}

#[test]
fn a_fragment_cut_off_anywhere_does_not_run_off_the_end() {
    // Not the sanitiser's shape, but `render_message_html` takes any string over the FFI, where a
    // panic is a crash: so every prefix of a fragment that exercises each part of the walk.
    let whole = r#"<style a="x">@media (prefers-color-scheme: dark){.c{color:#000}}</style><p style="color:#000;background:rgb(255 255 255)" bgcolor="white"><font color="black">é</font></p>"#;
    for (end, _) in whole.char_indices() {
        let _ = adapt(&whole[..end], MessageScheme::Dark);
        let _ = adapt(&whole[..end], MessageScheme::Light);
    }
}
