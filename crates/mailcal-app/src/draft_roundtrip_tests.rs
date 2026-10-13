//! The core's half of the contract that a draft reopens exactly as it was saved.
//!
//! A composer saves its document through [`rich_draft`], and a draft reopened from the server comes
//! back to it through [`sanitize_for_composer`]; the editor then reads that HTML back into a
//! document (`clients/composer/src/read_html.ts`). For the draft to reopen as it was, the last step
//! has to undo the first two, for every document the editor can hold, and neither language can
//! check that alone.
//!
//! So this test generates documents across the editor's whole schema, deterministically, passes
//! each through both core paths, and holds a fixture to the result: each entry's `document` and the
//! `html` the editor receives on reopening it. The editor's suite holds its reader to turning that
//! `html` back into the `document`. A change on this side fails here first; `MAILCAL_BLESS=1`
//! rewrites the fixture, and the editor's suite then says whether the reader still follows.

use std::path::PathBuf;

use engine_api::{DraftAttachmentDisposition, EmailAddress, InlinePart};
use mailcal_composer::ComposerDocument;
use serde_json::{Value, json};

use crate::{helpers::new_message_id, html::sanitize_for_composer, mail_compose::rich_draft};

const ENTRIES: usize = 80;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../clients/composer/tests/fixtures/reopened.json")
}

#[test]
fn the_reopened_fixture_holds_what_a_save_and_a_reopen_produce() {
    let mut rng = Rng(0x5eed_cafe_f00d_d00d);
    let entries: Vec<Value> = (0..ENTRIES)
        .map(|index| {
            let document = Generator::new(&mut rng).document();
            let html = reopened(&document);
            json!({ "name": format!("generated {index}"), "document": document, "html": html })
        })
        .collect();
    // One entry to a line: a diff of the fixture then names the documents that changed.
    let lines: Vec<String> = entries
        .iter()
        .map(|entry| serde_json::to_string(entry).expect("serialisable"))
        .collect();
    let written = format!("[\n{}\n]\n", lines.join(",\n"));
    let path = fixture_path();
    if std::env::var_os("MAILCAL_BLESS").is_some() {
        std::fs::write(&path, written).expect("the fixture is writable");
        return;
    }
    // Compared as values: whether a map keeps its keys in insertion order is a `serde_json` feature
    // another crate in the build can switch on, so the text can differ where the content does not.
    let held: Option<Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|held| serde_json::from_str(&held).ok());
    assert!(
        held == Some(Value::Array(entries)),
        "a save followed by a reopen no longer produces what clients/composer/tests/fixtures/\
         reopened.json holds; re-run with MAILCAL_BLESS=1, then run the editor's suite (`bun test` \
         in clients/composer)"
    );
}

/// What the editor is handed on reopening `document` after a save: the save's HTML, sanitised and
/// with its pictures inlined from the parts the save attached, exactly as a resume does.
fn reopened(document: &Value) -> String {
    let parsed: ComposerDocument =
        serde_json::from_value(document.clone()).expect("a generated document parses");
    let draft = rich_draft(
        new_message_id().expect("a Message-ID"),
        &EmailAddress::new("ada@example.test"),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        "Subject".to_owned(),
        parsed,
        Vec::new(),
        &[],
    )
    .expect("a generated document renders");
    let parts: Vec<InlinePart> = draft
        .attachments
        .iter()
        .filter_map(|attachment| match &attachment.disposition {
            DraftAttachmentDisposition::Inline { content_id } => Some(InlinePart::new(
                content_id.as_str(),
                attachment.media_type.clone(),
                attachment.content.clone(),
            )),
            DraftAttachmentDisposition::Attachment => None,
        })
        .collect();
    sanitize_for_composer(draft.html_body.as_deref().unwrap_or_default(), &parts)
}

/// xorshift64: enough to vary the documents, and the same documents on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % bound as u64).expect("fits")
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// Words, including the characters HTML treats specially and ones no layout may touch.
const WORDS: &[&str] = &[
    "Agenda",
    "a",
    "naïve",
    "café",
    "日本語",
    "R&D",
    "<b>",
    "x>y",
    "zero\u{200B}width",
    "\"quoted\"",
    "it's",
    "🙂",
    "50%",
    "e.g.",
    "Ø",
];

/// What sits between words: one space, a run of them, a tab.
const GAPS: &[&str] = &[" ", " ", " ", "  ", "   ", "\t", " \t "];

/// Spaces at a line's ends, which only survive if the HTML says so.
const EDGES: &[&str] = &["", "", "", " ", "  ", "\t"];

/// The toolbar's four sizes, and sizes only another client's draft carries.
const SIZES: &[u16] = &[13, 15, 18, 24, 11, 21, 32];
/// Family lists as the document holds them: the toolbar's, and ones another client's draft names.
const FONTS: &[&str] = &[
    "Arial, Helvetica, sans-serif",
    "Georgia, serif",
    "Comic Sans MS",
    "Segoe UI Emoji, sans-serif",
];
const HEADINGS: &[&str] = &["H1", "H2", "H3", "H4", "H5", "H6"];
const ALIGNMENTS: &[&str] = &["Center", "Right", "Justify"];
/// The editor's own indent steps, and Outlook's quarter inch and 35.4pt.
const INDENTS: &[u16] = &[40, 80, 24, 47];
const COLOURS: &[&str] = &["#d93025", "#1a73e8", "#188038", "#000000"];
const LINKS: &[&str] = &["https://example.test/agenda", "mailto:ada@example.test"];

/// Quoted originals as the reading view hands them to the composer: already sanitised markup.
const QUOTES: &[&str] = &[
    "<p>The original message.</p>",
    "<div>Line one<br>Line two</div><p>Kind regards,</p>",
    "<p>On Monday, Bob wrote:</p><blockquote style=\"margin:0 0 0 0.8ex;border-left:2px solid \
     #cccccc;padding-left:1ex\"><p>An earlier reply.</p></blockquote>",
    "<p>Thanks</p><div class=\"allodia-signature\"><p>Bob</p></div>",
    "<style>p { color: #333333; }</style><p>Styled in its own sheet.</p>",
    "<div class=\"aq-plain\" style=\"white-space: pre-wrap;\">Plain line\n  indented line\n\n&gt; \
     quoted line</div>",
    "<table><tbody><tr><td>Cell</td><td>Other</td></tr></tbody></table>",
];

/// Signatures as Settings stores them: already sanitised markup.
const SIGNATURES: &[&str] = &[
    "<p><strong>Ada Lovelace</strong></p><p>Analytical Engines<br>London</p>",
    "<p>Ada</p><blockquote>A motto, set off.</blockquote>",
    "<p><em>Sent with care</em></p>",
];

/// Pictures small enough to keep the fixture readable: a red, a green and a blue pixel.
const PICTURES: &[&str] = &[
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==",
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M/wHwAEBgIApD5fRAAAAABJRU5ErkJggg==",
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPj/HwADBwIAMCbHYQAAAABJRU5ErkJggg==",
];

/// One generated document, its pictures numbered in the order the editor will meet them.
struct Generator<'a> {
    rng: &'a mut Rng,
    attachments: Vec<Value>,
}

impl<'a> Generator<'a> {
    fn new(rng: &'a mut Rng) -> Self {
        Self {
            rng,
            attachments: Vec::new(),
        }
    }

    fn document(mut self) -> Value {
        let mut blocks = Vec::new();
        for _ in 0..=self.rng.below(6) {
            let block = match self.rng.below(11) {
                0..=4 => self.paragraph(),
                5 | 6 => json!({ "List": self.list(0) }),
                7 => self.table(),
                8 => self.quote(),
                9 => self.signature(),
                _ => json!({ "Rule": {} }),
            };
            blocks.push(block);
        }
        json!({ "blocks": blocks, "attachments": self.attachments })
    }

    fn paragraph(&mut self) -> Value {
        let mut paragraph = json!({ "content": self.line() });
        if self.rng.chance(10) {
            paragraph["heading"] = json!(self.rng.pick(HEADINGS));
        } else if self.rng.chance(35) {
            paragraph["spaced"] = json!(true);
        }
        if self.rng.chance(15) {
            paragraph["align"] = json!(self.rng.pick(ALIGNMENTS));
        }
        if self.rng.chance(15) {
            paragraph["indent_px"] = json!(self.rng.pick(INDENTS));
        }
        json!({ "Paragraph": paragraph })
    }

    /// A line's inline content, in the editor's one form for it: no two neighbouring runs with the
    /// same marks, and no empty run.
    fn line(&mut self) -> Vec<Value> {
        if self.rng.chance(10) {
            return Vec::new();
        }
        let mut content: Vec<Value> = Vec::new();
        let pieces = 1 + self.rng.below(4);
        for piece in 0..pieces {
            if self.rng.chance(12) {
                content.push(self.picture());
                continue;
            }
            let mut text = String::new();
            if piece == 0 {
                text.push_str(self.rng.pick(EDGES));
            }
            for word in 0..=self.rng.below(3) {
                if word > 0 {
                    text.push_str(self.rng.pick(GAPS));
                }
                text.push_str(self.rng.pick(WORDS));
            }
            if piece + 1 < pieces || self.rng.chance(30) {
                text.push_str(self.rng.pick(GAPS));
            }
            if piece + 1 == pieces {
                text.push_str(self.rng.pick(EDGES));
            }
            // A run that is nothing but a space, formatted on its own, between two others.
            if piece > 0 && self.rng.chance(10) {
                push_run(&mut content, run(" ", self.marks()));
            }
            push_run(&mut content, run(&text, self.marks()));
        }
        content
    }

    fn marks(&mut self) -> Value {
        let mut marks = json!({
            "bold": self.rng.chance(30),
            "italic": self.rng.chance(20),
            "underline": self.rng.chance(15),
        });
        if self.rng.chance(20) {
            marks["font_size"] = json!(self.rng.pick(SIZES));
        }
        if self.rng.chance(15) {
            marks["font"] = json!(self.rng.pick(FONTS));
        }
        if self.rng.chance(15) {
            marks["color"] = json!(self.rng.pick(COLOURS));
        }
        if self.rng.chance(10) {
            marks["highlight"] = json!(self.rng.pick(COLOURS));
        }
        if self.rng.chance(15) {
            marks["link"] = json!(self.rng.pick(LINKS));
        }
        marks
    }

    fn picture(&mut self) -> Value {
        let index = self.attachments.len();
        let id = format!("captured-{index}");
        self.attachments.push(json!({
            "id": id,
            "file_name": format!("image-{index}.png"),
            "media_type": "image/png",
            "size": null,
            "disposition": { "Inline": { "cid": format!("img{index}.fixture@allodia.local") } },
            "data_url": self.rng.pick(PICTURES),
        }));
        let width = if self.rng.chance(50) {
            json!(1 + self.rng.below(640))
        } else {
            Value::Null
        };
        let alt = *self.rng.pick(&["", "A chart", "Logo & mark"]);
        json!({ "Image": { "attachment_id": id, "alt_text": alt, "width_px": width } })
    }

    fn list(&mut self, depth: usize) -> Value {
        let kind = *self.rng.pick(&["Bullet", "Ordered"]);
        let items: Vec<Value> = (0..=self.rng.below(3))
            .map(|_| {
                // The item's own line first: its pictures come before its child list's.
                let content = self.line();
                let child = if depth < 3 && self.rng.chance(30) {
                    self.list(depth + 1)
                } else {
                    Value::Null
                };
                json!({ "content": content, "child": child })
            })
            .collect();
        json!({ "kind": kind, "items": items })
    }

    fn table(&mut self) -> Value {
        let columns = 1 + self.rng.below(3);
        let rows: Vec<Value> = (0..=self.rng.below(2))
            .map(|_| {
                let cells: Vec<Value> = (0..columns)
                    .map(|_| json!({ "content": self.line() }))
                    .collect();
                json!({ "cells": cells })
            })
            .collect();
        json!({ "Table": { "rows": rows } })
    }

    /// A quoted original in either style, with the attribution that style shows. Reopening keeps
    /// the shown one; the other is rebuilt from the original message, which this test does not
    /// have.
    fn quote(&mut self) -> Value {
        let body = *self.rng.pick(QUOTES);
        let (style, line, headers) = if self.rng.chance(50) {
            (
                "Indented",
                "On 1 March, Bob <bob@example.test> wrote:",
                json!([]),
            )
        } else {
            let headers = json!([
                { "label": "From", "value": "Bob <bob@example.test>" },
                { "label": "Subject", "value": "R&D <plan>" },
            ]);
            ("LineAndHeader", "", headers)
        };
        json!({ "Quote": {
            "style": style,
            "attribution": { "line": line, "headers": headers },
            "body_html": body,
            "body_plain": "",
        } })
    }

    fn signature(&mut self) -> Value {
        let mut body = (*self.rng.pick(SIGNATURES)).to_owned();
        if self.rng.chance(40) {
            body.push_str("<p><img src=\"");
            body.push_str(self.rng.pick(PICTURES));
            body.push_str("\" alt=\"Logo\"></p>");
        }
        json!({ "Signature": { "body_html": body, "body_plain": "" } })
    }
}

fn run(text: &str, marks: Value) -> Value {
    let mut run = marks;
    run["text"] = json!(text);
    json!({ "Text": run })
}

/// Appends `inline`, merging it into the run before it when the two carry the same marks.
fn push_run(content: &mut Vec<Value>, inline: Value) {
    if let Some(last) = content.last_mut()
        && same_marks(&last["Text"], &inline["Text"])
    {
        let text = format!(
            "{}{}",
            last["Text"]["text"].as_str().unwrap_or_default(),
            inline["Text"]["text"].as_str().unwrap_or_default()
        );
        last["Text"]["text"] = json!(text);
        return;
    }
    content.push(inline);
}

fn same_marks(a: &Value, b: &Value) -> bool {
    if a.is_null() || b.is_null() {
        return false;
    }
    [
        "bold",
        "italic",
        "underline",
        "font_size",
        "font",
        "color",
        "highlight",
        "link",
    ]
    .iter()
    .all(|key| a.get(key) == b.get(key))
}
