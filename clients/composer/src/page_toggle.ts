// The toolbar's sun or moon (`docs/reading-dark-mode.md`, rule 3): the page the message is written
// on, one press from the other one.
//
// It is a door to the reader's one dark-messages preference, the same one the reading view's
// toggle and Settings set, so pressing it only tells the host which page was asked for; the host
// stores that and names the page back (`setComposerPage`), as it does for every other change. It is
// drawn only where there is something to switch: the host has named a page, the app is dark, and
// the host listens for the choice.

import type { Labels } from "./labels";
import type { PageScheme } from "./page";

export interface PageToggle {
  /// Draws it, or takes it away, for the page, the app's scheme and the labels as they are now.
  update(): void;
}

/// `page` is the page in force, `appDark` whether the app is dark, `heard` whether the host listens
/// for the choice, `choose` tells it the page asked for, and `fit` lays the row out again when
/// the toggle comes or goes.
export function installPageToggle(
  root: HTMLElement,
  page: () => PageScheme | null,
  appDark: () => boolean,
  heard: () => boolean,
  labels: () => Labels,
  choose: (scheme: PageScheme) => void,
  fit: () => void,
): PageToggle {
  const doc = root.ownerDocument;
  const button = doc.getElementById("page-background") as HTMLButtonElement | null;
  const separator = doc.getElementById("page-background-sep");
  if (!button || !separator) return { update() {} };

  button.addEventListener("click", () => {
    const scheme = page();
    if (scheme) choose(scheme === "dark" ? "light" : "dark");
  });

  return {
    update() {
      const scheme = page();
      const shown = scheme !== null && appDark() && heard();
      const label = scheme === "dark" ? labels().lightBackground : labels().darkBackground;
      button.title = label;
      button.setAttribute("aria-label", label);
      button.dataset.page = scheme ?? "light";
      if (button.hidden === !shown) return;
      button.hidden = !shown;
      separator.hidden = !shown;
      fit();
    },
  };
}
