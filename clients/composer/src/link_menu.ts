// The link editor: the toolbar popover that makes, changes and removes a link.
//
// Two fields, the words and the address, the way Outlook asks. It opens filled from what the
// selection is on: an existing link's words and target, or the selected text. The selection is
// captured before either field takes focus, because typing into one moves the caret out of the
// message, and it is put back before anything is applied.

import { type SavedSelection, focusEditor, rangeWithin, restoreSelection, saveSelection } from "./dom";
import { type Labels } from "./labels";
import { applyLink, linkAtCaret, normalizeLinkAddress, removeLink, selectionHasLink } from "./links";

export function buildLinkMenu(
  panel: HTMLElement,
  editor: HTMLElement,
  doc: Document,
  labels: Labels,
  close: () => void,
): void {
  panel.textContent = "";
  const saved: SavedSelection | null = saveSelection(editor);
  const existing = linkAtCaret(editor);
  const selected = existing ? (existing.textContent ?? "") : (rangeWithin(editor)?.toString() ?? "");

  const field = (label: string, value: string, name: string) => {
    const wrapper = doc.createElement("label");
    wrapper.className = "field";
    const caption = doc.createElement("span");
    caption.textContent = label;
    const input = doc.createElement("input");
    input.type = "text";
    input.name = name;
    input.value = value;
    input.spellcheck = false;
    input.autocomplete = "off";
    wrapper.append(caption, input);
    panel.appendChild(wrapper);
    return input;
  };

  const text = field(labels.linkText, selected, "link-text");
  const address = field(
    labels.linkAddress,
    existing?.getAttribute("href")?.replace(/^mailto:/i, "") ?? "",
    "link-address",
  );
  address.inputMode = "url";

  const actions = doc.createElement("div");
  actions.className = "actions";
  const button = (label: string, run: () => void, primary = false) => {
    const node = doc.createElement("button");
    node.type = "button";
    node.textContent = label;
    if (primary) node.className = "primary";
    node.addEventListener("click", run);
    actions.appendChild(node);
    return node;
  };

  const dismiss = () => {
    close();
    focusEditor(editor);
    restoreSelection(editor, saved);
  };
  const apply = () => {
    const href = normalizeLinkAddress(address.value);
    if (!href) {
      address.setAttribute("aria-invalid", "true");
      address.focus();
      return;
    }
    close();
    const words = text.value.trim() === selected.trim() ? "" : text.value;
    applyLink(editor, saved, existing, { href, text: words });
  };

  if (existing || selectionHasLink(editor, saved)) {
    button(labels.linkRemove, () => {
      close();
      removeLink(editor, saved);
    });
  }
  const applyButton = button(labels.linkApply, apply, true);
  panel.appendChild(actions);

  const sync = () => {
    address.removeAttribute("aria-invalid");
    applyButton.disabled = address.value.trim() === "";
  };
  address.addEventListener("input", sync);
  sync();

  for (const input of [text, address]) {
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        apply();
      } else if (event.key === "Escape") {
        // The document-level handler closes the popover too; this also puts the caret back.
        event.preventDefault();
        dismiss();
      }
    });
  }

  // After the popover is shown: a field in a hidden panel cannot take focus.
  doc.defaultView?.setTimeout(() => address.focus(), 0);
}
