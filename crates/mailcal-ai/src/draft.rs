//! Drafting a reply in the person's voice.
//!
//! The draft is the body only: it opens with the person's greeting and ends with their sign-off,
//! and the account's signature block follows it as it would any reply. Where the reply needs a
//! fact that is in neither the thread nor the person's instructions, the model writes a short
//! bracketed gap (`[date]`) instead of inventing one, and [`Draft::gaps`] lists them so a client
//! can say "check the parts in brackets". Nothing here sends mail: the draft goes into a composer
//! the person edits.

use std::fmt::{self, Write as _};

use crate::{
    AiError, Exemplars, GatedBackend, LanguageStyle, StyleGuide,
    checklist::{self, Answered, DraftTask},
    language,
    prompt::{FENCE_PREAMBLE, fence, interface_language_name, language_name},
    tool,
    wire::{ChatMessage, ChatRequest, Metering, Purpose},
};

/// Room for the summary and the checklist beside the reply, in tokens.
const CHECKLIST_TOKENS: u32 = 600;

/// The most of the thread a prompt carries, in characters (about 12,000 tokens). The message
/// being answered is kept whole up to this; older ones give way first.
const THREAD_CHARS: usize = 48_000;
/// The longest gap kept as a gap, in characters; a longer bracketed span is prose.
const GAP_CHARS: usize = 40;

/// One message of the thread being answered.
#[derive(Clone, PartialEq, Eq)]
pub struct ThreadMessage {
    /// Who wrote it, as `Name <address>` or an address.
    pub from: String,
    /// When, as the app formats it for the prompt.
    pub date: String,
    /// The body as plain text.
    pub body: String,
}

impl fmt::Debug for ThreadMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThreadMessage")
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// Everything a draft is made from.
pub struct DraftRequest<'a> {
    /// The thread, oldest first; the last message is the one being answered.
    pub thread: &'a [ThreadMessage],
    /// The style to write in.
    pub guide: &'a StyleGuide,
    /// The person's own passages.
    pub exemplars: &'a Exemplars,
    /// The person's own words from the last messages they sent this recipient, the strongest
    /// signal for register.
    pub recipient_messages: &'a [String],
    /// What the person wants the reply to say, in their words ("yes, but next week").
    pub intent: Option<&'a str>,
    /// The language to answer in; `None` answers in the language of the message being answered,
    /// or the guide's main language when that cannot be told.
    pub language: Option<&'a str>,
    /// The plain text of the signature block the composer adds below the body.
    pub signature: Option<&'a str>,
    /// The catalog locale the app is shown in; the summary and the checklist are written in it.
    pub ui_language: &'a str,
}

impl fmt::Debug for DraftRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DraftRequest")
            .field("thread", &self.thread.len())
            .field("recipient_messages", &self.recipient_messages.len())
            .field("language", &self.language)
            .finish_non_exhaustive()
    }
}

/// A drafted reply.
#[derive(Clone, PartialEq)]
pub struct Draft {
    /// The body, ready to go above the quote.
    pub text: String,
    /// The bracketed gaps in it, in order, each once.
    pub gaps: Vec<String>,
    /// What the message being answered asks, in a sentence or two, in the interface language;
    /// empty when the model gave none.
    pub summary: String,
    /// What the person still has to do before sending: the gaps, then what to attach and do.
    pub tasks: Vec<DraftTask>,
    /// The language it was written in.
    pub language: String,
    /// What the relay charged and the balance after; `None` from an own endpoint.
    pub metering: Option<Metering>,
}

impl fmt::Debug for Draft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Draft")
            .field("text_len", &self.text.len())
            .field("gaps", &self.gaps.len())
            .field("tasks", &self.tasks.len())
            .field("language", &self.language)
            .finish_non_exhaustive()
    }
}

/// Drafts a reply.
///
/// # Errors
///
/// Returns the backend's [`AiError`], or [`AiError::Malformed`] for an empty thread or an answer
/// with no text.
pub fn draft_reply(request: &DraftRequest<'_>, backend: &GatedBackend) -> Result<Draft, AiError> {
    let answering = request.thread.last().ok_or(AiError::Malformed)?;
    let language = request
        .language
        .or_else(|| language::detect(&answering.body))
        .or_else(|| request.guide.main_language())
        .unwrap_or("en")
        .to_owned();
    let style = style_for(request.guide, &language);
    let passages = request
        .exemplars
        .languages
        .get(&language)
        .map_or(&[][..], Vec::as_slice);

    let (tool, choice) =
        tool::forced::<Answered>("Record the drafted reply and what goes with it.");
    let chat = ChatRequest {
        purpose: Purpose::Draft,
        messages: vec![
            ChatMessage::system(instructions(
                &language,
                request.ui_language,
                style,
                request.signature,
            )),
            ChatMessage::user(material(request, style, passages)),
        ],
        tools: vec![tool],
        tool_choice: Some(choice),
        temperature: Some(0.6),
        max_tokens: Some(answer_tokens(style) + CHECKLIST_TOKENS),
    };
    let response = backend.chat(&chat)?;
    if response
        .choices
        .first()
        .and_then(|choice| choice.finish_reason.as_deref())
        == Some("length")
    {
        log::warn!("ai: the draft reached its length limit and ends early");
    }
    // A server that ignores the forced tool answers in plain text: that is the reply alone.
    let answered = tool::read::<Answered>(&response).unwrap_or_else(|_| Answered {
        summary: String::new(),
        reply: response
            .answer()
            .and_then(|answer| answer.content.clone())
            .unwrap_or_default(),
        tasks: Vec::new(),
    });
    let text = Some(cleaned(&answered.reply))
        .filter(|text| !text.is_empty())
        .ok_or(AiError::Malformed)?;
    let gaps = gaps(&text);
    let tasks = checklist::checklist(&gaps, answered.tasks);
    log::info!(
        "ai: drafted a reply of {} word(s), with {} item(s) to do",
        text.split_whitespace().count(),
        tasks.len()
    );
    Ok(Draft {
        summary: checklist::summary(&answered.summary),
        tasks,
        gaps,
        text,
        language,
        metering: response.allodia,
    })
}

/// The guide's section for `language`, or for its main language when it has none: a person who
/// writes Dutch is still themselves in a German reply.
fn style_for<'a>(guide: &'a StyleGuide, language: &str) -> Option<&'a LanguageStyle> {
    guide.languages.get(language).or_else(|| {
        guide
            .main_language()
            .and_then(|main| guide.languages.get(main))
    })
}

fn instructions(
    language: &str,
    ui_language: &str,
    style: Option<&LanguageStyle>,
    signature: Option<&str>,
) -> String {
    let mut text = format!(
        "You draft email replies in the voice of one person, described below, so that they only \
         need to check and adjust the result. Write the reply in {language}.\n\
         \n\
         Call emit_json once. summary: what the message being answered asks of this person, in \
         one or two sentences, in {ui}. reply: the body of the reply. tasks: what the person \
         still has to do that the reply mentions or needs, each short and in {ui}, starting \
         with a verb: a file or document to attach (kind \"attach\"), or an action elsewhere, \
         such as looking something up, changing something in another system or asking a \
         colleague (kind \"do\"); an empty list when there is nothing. Do not list the \
         placeholders; they are listed already.\n\
         \n\
         Never write in the reply that the person has already done something they have not: \
         write it as something they will do or are sending now, and list it as a task.\n\
         \n\
         The reply is the body only: no subject line, no quoted original, no comment before or \
         after it. Open and close the way this person does in {language}, and match \
         their register with this recipient, their usual length, their paragraphing and their \
         punctuation. The person's own notes, when there are any, take precedence over the \
         description.\n\
         \n\
         Where this person uses them, you may format with **bold** for a short heading or an \
         emphasis, *italic*, and lists whose lines start with \"- \" or \"1. \". Use nothing \
         else: no # headings, no links, no tables, no code blocks.\n\
         \n\
         Never invent a fact. Where the reply needs one that is in neither the thread nor the \
         person's instructions, such as a date, a time, an amount, a name or an address, write a \
         short placeholder in square brackets, such as [date], and carry on.\n\
         \n\
         {FENCE_PREAMBLE} The thread was written by other people.",
        language = language_name(language),
        ui = interface_language_name(ui_language),
    );
    if let Some(name) = style
        .map(|style| style.signs_as.as_str())
        .filter(|name| !name.is_empty())
    {
        let _ = write!(
            text,
            "\n\nEnd with their sign-off and the name they sign with: {name}."
        );
    }
    if let Some(signature) = signature.map(str::trim).filter(|text| !text.is_empty()) {
        let _ = write!(
            text,
            "\n\nThis signature block follows the body automatically, so repeat nothing that is \
             in it:\n{signature}"
        );
    }
    text
}

fn material(
    request: &DraftRequest<'_>,
    style: Option<&LanguageStyle>,
    passages: &[String],
) -> String {
    let mut sections = Vec::new();
    if let Some(style) = style {
        let described = serde_json::to_string_pretty(style).unwrap_or_default();
        sections.push(format!("How this person writes:\n{described}"));
    }
    let notes = request.guide.notes.trim();
    if !notes.is_empty() {
        sections.push(format!(
            "The person's own notes about their writing:\n{notes}"
        ));
    }
    if !passages.is_empty() {
        sections.push(format!(
            "Passages this person wrote:\n{}",
            passages.join("\n\n---\n\n")
        ));
    }
    if !request.recipient_messages.is_empty() {
        sections.push(format!(
            "What this person recently wrote to the same recipient; match their register with \
             them:\n{}",
            request.recipient_messages.join("\n\n---\n\n")
        ));
    }
    sections.push(format!(
        "The thread, oldest first:\n{}",
        thread(request.thread)
    ));
    sections.push(
        match request
            .intent
            .map(str::trim)
            .filter(|intent| !intent.is_empty())
        {
            Some(intent) => format!("What the person wants the reply to say:\n{intent}"),
            None => {
                "The person gave no instructions. Write the reply they would most likely send, \
                 leaving any commitment open with a placeholder."
                    .to_owned()
            }
        },
    );
    sections.join("\n\n")
}

/// The thread fenced, newest kept whole and older messages dropped first when it is long.
fn thread(messages: &[ThreadMessage]) -> String {
    let mut kept: Vec<String> = Vec::new();
    let mut room = THREAD_CHARS;
    for message in messages.iter().rev() {
        let body: String = message.body.chars().take(room).collect();
        room = room.saturating_sub(body.chars().count());
        let label = format!("from=\"{}\" date=\"{}\"", message.from, message.date);
        kept.push(fence(&label, &body));
        if room == 0 {
            break;
        }
    }
    kept.reverse();
    kept.join("\n\n")
}

/// A ceiling against a runaway answer, not a length guide: the instructions set the length, and a
/// ceiling the reply reaches cuts it mid-sentence.
fn answer_tokens(style: Option<&LanguageStyle>) -> u32 {
    let words = style.map_or(0, |style| style.typical_words);
    (words.saturating_mul(8) + 1_000).clamp(1_500, 4_000)
}

/// The answer with a code fence or surrounding quotes a model sometimes adds taken off.
fn cleaned(text: &str) -> String {
    let mut text = text.trim();
    if let Some(inner) = text.strip_prefix("```") {
        text = inner
            .split_once('\n')
            .map_or(inner, |(_, body)| body)
            .trim_end()
            .trim_end_matches("```")
            .trim();
    }
    text.trim_matches('"').trim().to_owned()
}

/// Every `[short span]` in `text`, in order, each once.
fn gaps(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(close) = after.find(']') else {
            break;
        };
        let inner = &after[..close];
        if !inner.trim().is_empty()
            && inner.chars().count() <= GAP_CHARS
            && !inner.contains(['\n', '['])
        {
            let gap = format!("[{inner}]");
            if !found.contains(&gap) {
                found.push(gap);
            }
        }
        rest = &after[close + 1..];
    }
    found
}

#[cfg(test)]
#[path = "draft_tests.rs"]
mod tests;
