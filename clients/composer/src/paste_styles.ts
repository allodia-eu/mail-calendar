// The marks a style sheet in pasted markup gives a class: the one thing read from a style sheet.
//
// Excel writes a cell's bold, italic and underline as a class (`.xl63 {font-weight:700}`) rather than
// on the cell, so the walk in `paste.ts` asks here as well as of the element's own style.

/// The style properties a mark is read from, and the only ones read from a style sheet.
export const MARK_PROPERTIES = ["font-weight", "font-style", "text-decoration-line", "text-decoration"];

/// A rule in the pasted markup's style sheet that gives a class one of the marks: `.xl63` or
/// `td.xl63`.
export interface ClassRule {
  tag: string | null;
  className: string;
  style: CSSStyleDeclaration;
}

/// The class rules in `source`'s style sheets that name a mark.
///
/// Excel writes every cell's bold, italic and underline as a class (`.xl63 {font-weight:700}`), and
/// a run inside a cell as `<font class="font5">`, so without these a sheet pastes with none of its
/// formatting. Each sheet is parsed into a stylesheet that is never applied, so nothing in it loads
/// or draws, and only `MARK_PROPERTIES` are read back. A selector counts when it is a single class,
/// alone or on one tag; one that depends on an element's place in the document is skipped, since
/// the walk does not keep that place.
export function classRules(view: Window & typeof globalThis, source: Document): ClassRule[] {
  if (typeof view.CSSStyleSheet !== "function") return [];
  const rules: ClassRule[] = [];
  for (const element of Array.from(source.querySelectorAll("style"))) {
    const sheet = new view.CSSStyleSheet();
    try {
      sheet.replaceSync(element.textContent ?? "");
    } catch {
      continue;
    }
    for (const rule of Array.from(sheet.cssRules)) {
      if (!("selectorText" in rule) || !("style" in rule)) continue;
      const style = rule.style as CSSStyleDeclaration;
      if (!MARK_PROPERTIES.some((property) => style.getPropertyValue(property))) continue;
      for (const selector of String(rule.selectorText).split(",")) {
        const match = /^([a-z][a-z0-9]*)?\.(-?[_a-z][\w-]*)$/i.exec(selector.trim());
        if (match) rules.push({ tag: match[1]?.toLowerCase() ?? null, className: match[2]!, style });
      }
    }
  }
  return rules;
}

/// `property` on `element` as the cascade would settle it among what is read: the inline style,
/// else the class rule that names it, a rule on the element's tag over a bare class and a later
/// rule over an earlier one.
export function declared(element: HTMLElement, property: string, classes: ClassRule[]): string {
  const inline = element.style?.getPropertyValue(property) ?? "";
  if (inline || classes.length === 0) return inline;
  let value = "";
  let rank = -1;
  for (const rule of classes) {
    if (rule.tag && rule.tag !== element.localName) continue;
    if (!element.classList?.contains(rule.className)) continue;
    const named = rule.style.getPropertyValue(property);
    const ruleRank = rule.tag ? 1 : 0;
    if (named && ruleRank >= rank) {
      value = named;
      rank = ruleRank;
    }
  }
  return value;
}
