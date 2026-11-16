// The toolbar on a desktop keeps to one row (`docs/composer-layout.md`, rule 7): the controls that
// do not fit move, last first, into the More panel at the row's end, as Outlook's do. They are the
// same elements, moved rather than copied, so a control behind More is the control itself, with its
// own handlers and state. On a touch screen the row scrolls sideways instead (native chrome), so
// every control stays in it and More is not drawn.

import { documentOf } from "./dom";

/// Whether the row is wider than the toolbar has room for. Read from the layout on a real page.
export interface OverflowMeasures {
  overflows(): boolean;
}

export interface Overflow {
  /// Puts every control back in the row, then moves into the panel what does not fit. Answers
  /// whether the arrangement changed.
  fit(): boolean;
}

/// `root` is the toolbar, `more` the More control's wrapper at its end, `panel` the panel it opens.
export function installOverflow(
  root: HTMLElement,
  more: HTMLElement,
  panel: HTMLElement,
  measures: OverflowMeasures = { overflows: () => root.scrollWidth > root.clientWidth + 0.5 },
): Overflow {
  const doc = documentOf(root);
  const controls = Array.from(root.children).filter((child) => child !== more) as HTMLElement[];
  let behind = 0;

  const isSeparator = (node: HTMLElement | undefined) => node?.classList.contains("sep") ?? false;

  return {
    fit() {
      const before = behind;
      for (const control of controls) root.insertBefore(control, more);
      more.hidden = true;
      behind = 0;
      if (!doc.body.classList.contains("native-chrome") && measures.overflows()) {
        more.hidden = false;
        const row = controls.slice();
        while (row.length > 0 && measures.overflows()) {
          panel.insertBefore(row.pop()!, panel.firstChild);
          // A separator never ends the row; one that opens the panel is not drawn (toolbar.css).
          while (isSeparator(row[row.length - 1])) panel.insertBefore(row.pop()!, panel.firstChild);
        }
        behind = controls.length - row.length;
      }
      return behind !== before;
    },
  };
}
