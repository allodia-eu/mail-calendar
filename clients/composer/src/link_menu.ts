// The link editor, which makes, changes and removes a link: the host's own dialog where the host
// answers `link` requests (`host_requests.ts`), and otherwise a popover under the toolbar button.
//
// Two fields, the words and the address, the way Outlook asks. It opens filled from what the
// selection is on: an existing link's words and target, or the selected text. The selection is
// captured before either field takes focus, because typing into one moves the caret out of the
// message, and it is put back before anything is applied.

import { type SavedSelection, focusEditor, rangeWithin, restoreSelection, saveSelection } from "./dom";
import { type HostRequests } from "./host_requests";
import { type Labels } from "./labels";
import {
  applyLink,
  linkAtCaret,
  normalizeLinkAddress,
  removeLink,
  safeLinkHref,
  selectionHasLink,
} from "./links";

/// What the link editor opens with, read before anything takes focus.
interface LinkStart {
  saved: SavedSelection | null;
  existing: HTMLAnchorElement | null;
  /// The selected words, or the existing link's.
  selected: string;
  /// The existing link's target as the user would type it: a mail link without its `mailto:`.
  address: string;
  removable: boolean;
}

function linkStart(editor: HTMLElement): LinkStart {
  const saved = saveSelection(editor);
  const existing = linkAtCaret(editor);
  return {
    saved,
    existing,
    selected: existing ? (existing.textContent ?? "") : (rangeWithin(editor)?.toString() ?? ""),
    address: existing?.getAttribute("href")?.replace(/^mailto:/i, "") ?? "",
    removable: Boolean(existing) || selectionHasLink(editor, saved),
  };
}

/// The words to show, or empty to keep the selected ones as they are.
function wordsFor(start: LinkStart, typed: string): string {
  return typed.trim() === start.selected.trim() ? "" : typed;
}

/// Asks the host's link dialog, then makes, changes or removes the link it answers with. The host
/// has completed and checked the address in the core already; it is checked again here, because
/// what reaches the document is this file's promise to keep.
export async function editLinkThroughHost(editor: HTMLElement, requests: HostRequests): Promise<void> {
  const start = linkStart(editor);
  const answer = await requests.request("link", {
    text: start.selected,
    address: start.address,
    removable: start.removable,
  });
  if (answer === "remove") {
    removeLink(editor, start.saved);
    return;
  }
  const href = answer === null || answer === "cancel" ? null : safeLinkHref(answer.apply.address);
  if (href === null || answer === null || answer === "cancel") {
    focusEditor(editor);
    restoreSelection(editor, start.saved);
    return;
  }
  applyLink(editor, start.saved, start.existing, { href, text: wordsFor(start, answer.apply.text) });
}

export function buildLinkMenu(
  panel: HTMLElement,
  editor: HTMLElement,
  doc: Document,
  labels: Labels,
  close: () => void,
): void {
  panel.textContent = "";
  const start = linkStart(editor);
  const { saved, existing, selected } = start;

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
  const address = field(labels.linkAddress, start.address, "link-address");
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
    applyLink(editor, saved, existing, { href, text: wordsFor(start, text.value) });
  };

  if (start.removable) {
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
