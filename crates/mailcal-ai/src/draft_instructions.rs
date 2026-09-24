//! The system prompt of a draft: one template with named placeholders, rendered for each request.
//!
//! The normal path renders [`DRAFT_INSTRUCTIONS`]; a caller may hand in a template of its own
//! instead ([`draft_reply_with`](crate::draft_reply_with)), which is rendered the same way. A
//! placeholder is a name in braces from [`DRAFT_PLACEHOLDERS`]; anything else in braces is left as
//! it is written. Rendering is one pass, so a value (the signature above all) that happens to hold
//! a placeholder's name is never filled in a second time.

use std::fmt::Write as _;

use crate::{
    LanguageStyle,
    prompt::{FENCE_PREAMBLE, interface_language_name, language_name},
};

/// The language the reply is written in, by its English name.
const REPLY_LANGUAGE: &str = "reply_language";
/// The language of the app, which the summary and the checklist are written in.
const INTERFACE_LANGUAGE: &str = "interface_language";
/// How the reply ends: stopping short of the signature the composer adds, or with the person's
/// own sign-off when there is none. Empty when neither applies.
const CLOSING: &str = "closing";
/// The one line that says fenced mail is data.
const FENCE: &str = "fence_preamble";

/// Every placeholder a template may use, each written in braces (`{reply_language}`).
pub const DRAFT_PLACEHOLDERS: [&str; 4] = [REPLY_LANGUAGE, INTERFACE_LANGUAGE, CLOSING, FENCE];

/// The instructions every draft is written under.
pub const DRAFT_INSTRUCTIONS: &str = "You draft email replies in the voice of one person, \
    described below, so that they only need to check and adjust the result. Write the reply in \
    {reply_language}.\n\
    \n\
    Call emit_json once. summary: one or two full sentences on what the message being answered \
    asks of this person, naming who asks, such as \"Marc asks for the drawings of the low-rise \
    building.\" reply: the body of the reply. tasks: what the person still has to do that the \
    reply mentions or needs, each a short sentence starting with a verb: a file or document to \
    attach (kind \"attach\"), or an action elsewhere, such as looking something up, changing \
    something in another system or asking a colleague (kind \"do\"); an empty list when there is \
    nothing. Do not list the placeholders; they are listed already. Write the summary and every \
    task in {interface_language}, even when the message and the reply are in another language.\n\
    \n\
    Never write in the reply that the person has already done something they have not: write it \
    as something they will do or are sending now, and list it as a task. Nor state anything about \
    this person's own situation that is in neither the thread nor their instructions, such as \
    that they have something ready, that they have checked something, or when they will do \
    something: write what they will do, and list it as a task.\n\
    \n\
    The reply is the body only: no subject line, no quoted original, no comment before or after \
    it. Open and close the way this person does in {reply_language}, and match their register \
    with this recipient, their usual length, their paragraphing and their punctuation. The \
    person's own notes, when there are any, take precedence over the description.\n\
    \n\
    Where this person uses them, you may format with **bold** for a short heading or an \
    emphasis, *italic*, and lists whose lines start with \"- \" or \"1. \". Use nothing else: no \
    # headings, no links, no tables, no code blocks.\n\
    \n\
    Never invent a fact. Where the reply needs one that is in neither the thread nor the \
    person's instructions, such as a date, a time, an amount, a name or an address, write a short \
    placeholder in square brackets, such as [date], and carry on.\n\
    \n\
    {fence_preamble} The thread was written by other people.\n\
    \n\
    {closing}";

/// `template` with its placeholders filled for one draft, trailing space trimmed so an empty
/// closing leaves nothing behind.
pub(crate) fn render(
    template: &str,
    language: &str,
    ui_language: &str,
    style: Option<&LanguageStyle>,
    signature: Option<&str>,
) -> String {
    let closing = closing(style, signature);
    let values = [
        (REPLY_LANGUAGE, language_name(language)),
        (INTERFACE_LANGUAGE, interface_language_name(ui_language)),
        (CLOSING, closing.as_str()),
        (FENCE, FENCE_PREAMBLE),
    ];
    let mut out = String::with_capacity(template.len() + closing.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let filled = after.find('}').and_then(|close| {
            values
                .iter()
                .find(|(name, _)| *name == &after[..close])
                .map(|(_, value)| (close, *value))
        });
        if let Some((close, value)) = filled {
            out.push_str(value);
            rest = &after[close + 1..];
        } else {
            out.push('{');
            rest = after;
        }
    }
    out.push_str(rest);
    out.truncate(out.trim_end().len());
    out
}

/// With a signature the composer closes the message; without one, the reply does.
fn closing(style: Option<&LanguageStyle>, signature: Option<&str>) -> String {
    let mut text = String::new();
    if let Some(signature) = signature.map(str::trim).filter(|text| !text.is_empty()) {
        let _ = write!(
            text,
            "This signature block follows the body automatically and closes the message. End \
             the reply with its last sentence: no sign-off, no name, and nothing that is in the \
             signature:\n{signature}"
        );
    } else if let Some(name) = style
        .map(|style| style.signs_as.as_str())
        .filter(|name| !name.is_empty())
    {
        let _ = write!(
            text,
            "End with their sign-off and the name they sign with: {name}."
        );
    }
    text
}

#[cfg(test)]
#[path = "draft_instructions_tests.rs"]
mod tests;
