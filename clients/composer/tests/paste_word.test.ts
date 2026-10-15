import { describe, expect, test } from "bun:test";

import { documentBlocks } from "../src/document";
import { pastedHtml } from "../src/paste";
import type { Block, InlineContent, ListValue } from "../src/types";
import { harness } from "./support";

function pasted(html: string): Block[] {
  const { editor } = harness("");
  const markup = pastedHtml(editor.ownerDocument, html);
  if (markup === null) throw new Error("nothing was kept");
  editor.innerHTML = markup;
  return documentBlocks(editor);
}

function text(content: InlineContent[]): string {
  return content.map((inline) => ("Text" in inline ? inline.Text.text : "")).join("");
}

function list(block: Block | undefined): ListValue {
  if (!block || !("List" in block)) throw new Error(`expected a list, got ${JSON.stringify(block)}`);
  return block.List;
}

/// One list paragraph the way Word writes it: the marker is text inside a `mso-list:Ignore` span,
/// behind a conditional comment, and the level is in the paragraph's own style.
function item(list: string, level: number, marker: string, body: string): string {
  return (
    `<p class=MsoListParagraphCxSpMiddle style='margin-left:${level * 0.5}in;text-indent:-.25in;` +
    `mso-list:${list} level${level} lfo1'><![if !supportLists]><span style='font-family:Symbol'>` +
    `<span style='mso-list:Ignore'>${marker}<span style='font:7.0pt "Times New Roman"'>` +
    `&nbsp;&nbsp;&nbsp;&nbsp;&nbsp; </span></span></span><![endif]>${body}<o:p></o:p></p>`
  );
}

describe("a list pasted from Word", () => {
  test("its paragraphs become a list, without the bullet Word draws as text", () => {
    const [before, bullets, after] = pasted(
      "<p class=MsoNormal>Before<o:p></o:p></p>" +
        item("l0", 1, "·", "First") +
        item("l0", 1, "·", "<b>Second</b>") +
        "<p class=MsoNormal>After<o:p></o:p></p>",
    );
    expect(before).toHaveProperty("Paragraph");
    const items = list(bullets).items;
    expect(list(bullets).kind).toBe("Bullet");
    expect(items.map((entry) => text(entry.content))).toEqual(["First", "Second"]);
    expect(after).toHaveProperty("Paragraph");
  });

  test("a numbered list is ordered, and a deeper level is the previous item's sub-list", () => {
    const [block] = pasted(
      item("l1", 1, "1.", "One") +
        item("l1", 2, "a.", "One a") +
        item("l1", 2, "b.", "One b") +
        item("l1", 1, "2.", "Two"),
    );
    const outer = list(block);
    expect(outer.kind).toBe("Ordered");
    expect(outer.items.map((entry) => text(entry.content))).toEqual(["One", "Two"]);
    expect(outer.items[0]!.child!.kind).toBe("Ordered");
    expect(outer.items[0]!.child!.items.map((entry) => text(entry.content))).toEqual(["One a", "One b"]);
  });

  test("a bullet list nested in a numbered one keeps both kinds", () => {
    const [block] = pasted(item("l2", 1, "1.", "Step") + item("l2", 2, "o", "detail") + item("l2", 1, "2.", "Next"));
    const outer = list(block);
    expect(outer.kind).toBe("Ordered");
    expect(outer.items[0]!.child!.kind).toBe("Bullet");
  });

  test("an outline numbered 1., 1.1., 2. is ordered at every level", () => {
    const [block] = pasted(item("l4", 1, "1.", "One") + item("l4", 2, "1.1.", "One one") + item("l4", 1, "2.", "Two"));
    expect(list(block).items[0]!.child!.kind).toBe("Ordered");
  });

  test("a heading Word numbers stays out of a list, and keeps its number", () => {
    const [heading, body] = pasted(
      "<h1 style='mso-list:l5 level1 lfo3'><![if !supportLists]><span style='mso-list:Ignore'>1." +
        "<span>&nbsp;&nbsp; </span></span><![endif]>Introduction</h1><p class=MsoNormal>Body</p>",
    );
    if (!heading || !("Paragraph" in heading)) throw new Error(`expected a paragraph, got ${JSON.stringify(heading)}`);
    expect(text(heading.Paragraph.content)).toMatch(/^1\.\s+Introduction$/);
    expect(body).toHaveProperty("Paragraph");
  });

  test("two lists with a paragraph between them stay two lists", () => {
    const blocks = pasted(item("l0", 1, "·", "a") + "<p class=MsoNormal>between</p>" + item("l3", 1, "1.", "b"));
    expect(blocks.map((block) => Object.keys(block)[0])).toEqual(["List", "Paragraph", "List"]);
    expect(list(blocks[2]).kind).toBe("Ordered");
  });

  test("Word's own clipboard: a bulleted list, then a numbered one with a lettered level", () => {
    const blocks = pasted(WORD_LISTS);
    expect(blocks.map((block) => Object.keys(block)[0])).toEqual(["Paragraph", "List", "Paragraph", "List", "Paragraph"]);
    const bullets = list(blocks[1]);
    expect(bullets.kind).toBe("Bullet");
    expect(bullets.items.map((entry) => text(entry.content))).toEqual(["Apples", "Green ones", "Pears"]);
    const pears = bullets.items[2]!.content[0]!;
    expect("Text" in pears && pears.Text.bold).toBe(true);
    const steps = list(blocks[3]);
    expect(steps.kind).toBe("Ordered");
    expect(steps.items.map((entry) => text(entry.content))).toEqual(["Wash", "Dry"]);
    expect(steps.items[0]!.child!.kind).toBe("Ordered");
    expect(steps.items[0]!.child!.items.map((entry) => text(entry.content))).toEqual(["Rinse twice"]);
  });

  test("Word for Mac's own clipboard: a list it holds as HTML is a list already, sub-list and all", () => {
    const blocks = pasted(WORD_MAC_LISTS);
    expect(blocks.map((block) => Object.keys(block)[0])).toEqual(["List", "Paragraph", "List"]);
    const bullets = list(blocks[0]);
    expect(bullets.kind).toBe("Bullet");
    expect(bullets.items.map((entry) => text(entry.content))).toEqual(["Apples", "Pears"]);
    const steps = list(blocks[2]);
    expect(steps.kind).toBe("Ordered");
    expect(steps.items.map((entry) => text(entry.content))).toEqual(["Wash", "Dry"]);
    expect(steps.items[0]!.child!.kind).toBe("Ordered");
    expect(steps.items[0]!.child!.items.map((entry) => text(entry.content))).toEqual(["Rinse twice"]);
  });
});

// Word for Windows (Microsoft 365) copying a document with a bulleted list and an outline-numbered
// list, the fragment between its StartFragment and EndFragment comments, as it wrote it.
const WORD_LISTS = `<p class=MsoNormal>Shopping:<o:p></o:p></p>

<p class=MsoListParagraphCxSpFirst style='text-indent:-.25in;mso-list:l0 level1 lfo1'><![if !supportLists]><span
style='font-family:Symbol;mso-fareast-font-family:Symbol;mso-bidi-font-family:
Symbol'><span style='mso-list:Ignore'>·<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
</span></span></span><![endif]>Apples<o:p></o:p></p>

<p class=MsoListParagraphCxSpMiddle style='margin-left:53.4pt;mso-add-space:
auto;text-indent:-.25in;mso-list:l0 level1 lfo1'><![if !supportLists]><span
style='font-family:Symbol;mso-fareast-font-family:Symbol;mso-bidi-font-family:
Symbol'><span style='mso-list:Ignore'>·<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
</span></span></span><![endif]>Green ones<o:p></o:p></p>

<p class=MsoListParagraphCxSpLast style='margin-left:.25in;mso-add-space:auto;
text-indent:-.25in;mso-list:l0 level1 lfo1'><![if !supportLists]><span
style='font-family:Symbol;mso-fareast-font-family:Symbol;mso-bidi-font-family:
Symbol'><span style='mso-list:Ignore'>·<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
</span></span></span><![endif]><b style='mso-bidi-font-weight:normal'>Pears</b><o:p></o:p></p>

<p class=MsoNormal>Steps:<o:p></o:p></p>

<p class=MsoListParagraphCxSpFirst style='margin-left:.25in;mso-add-space:auto;
text-indent:-.25in;mso-list:l1 level1 lfo2'><![if !supportLists]><span
style='mso-bidi-font-family:Aptos;mso-bidi-theme-font:minor-latin'><span
style='mso-list:Ignore'>1)<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
</span></span></span><![endif]>Wash<o:p></o:p></p>

<p class=MsoListParagraphCxSpMiddle style='text-indent:-.25in;mso-list:l1 level2 lfo2'><![if !supportLists]><span
style='mso-bidi-font-family:Aptos;mso-bidi-theme-font:minor-latin'><span
style='mso-list:Ignore'>a)<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
</span></span></span><![endif]>Rinse twice<o:p></o:p></p>

<p class=MsoListParagraphCxSpLast style='margin-left:.25in;mso-add-space:auto;
text-indent:-.25in;mso-list:l1 level1 lfo2'><![if !supportLists]><span
style='mso-bidi-font-family:Aptos;mso-bidi-theme-font:minor-latin'><span
style='mso-list:Ignore'>2)<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
</span></span></span><![endif]>Dry<o:p></o:p></p>

<p class=MsoNormal>Done.<o:p></o:p></p>`;

// Word for Mac (Microsoft 365) copying lists that came to it as HTML: it writes `<ul>`/`<ol>` again,
// with `mso-list` on every item and the lettered level as an `<ol>` straight inside the outer one.
const WORD_MAC_LISTS = `<ul type=disc>
 <li class=MsoNormal style='mso-margin-top-alt:auto;mso-margin-bottom-alt:auto;
     mso-list:l1 level1 lfo1;tab-stops:list 36.0pt'><span style='mso-fareast-font-family:
     "Times New Roman"'>Apples<o:p></o:p></span></li>
 <li class=MsoNormal style='mso-margin-top-alt:auto;mso-margin-bottom-alt:auto;
     mso-list:l1 level1 lfo1;tab-stops:list 36.0pt'><b><span style='mso-fareast-font-family:
     "Times New Roman"'>Pears</span></b><span style='mso-fareast-font-family:
     "Times New Roman"'><o:p></o:p></span></li>
</ul>

<p>Between<o:p></o:p></p>

<ol start=1 type=1>
 <li class=MsoNormal style='mso-margin-top-alt:auto;mso-margin-bottom-alt:auto;
     mso-list:l0 level1 lfo2;tab-stops:list 36.0pt'><span style='mso-fareast-font-family:
     "Times New Roman"'>Wash<o:p></o:p></span></li>
 <ol start=1 type=a>
  <li class=MsoNormal style='mso-margin-top-alt:auto;mso-margin-bottom-alt:
      auto;mso-list:l0 level2 lfo2;tab-stops:list 72.0pt'><span
      style='mso-fareast-font-family:"Times New Roman"'>Rinse twice<o:p></o:p></span></li>
 </ol>
 <li class=MsoNormal style='mso-margin-top-alt:auto;mso-margin-bottom-alt:auto;
     mso-list:l0 level1 lfo2;tab-stops:list 36.0pt'><span style='mso-fareast-font-family:
     "Times New Roman"'>Dry<o:p></o:p></span></li>
</ol>`;
