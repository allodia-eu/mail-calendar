// Sizes, lengths and typefaces as other clients' markup spells them, read into the one spelling the
// document holds.
//
// The editor writes pixels and a canonical family list. A draft Thunderbird or Outlook wrote says
// the same things as `<font size="5">`, `font-size:16.0pt`, `margin-left:.25in` and
// `font-family:"Comic Sans MS"`, and a draft reopened here has to keep each of them.

import { type FontSize, fontSizeOf } from "./types";

/// The pixel size of each HTML `<font size>` from 1 to 7: CSS's `x-small` to `xxx-large`, which is
/// how every engine draws them.
const LEGACY_SIZES = [10, 13, 16, 18, 24, 32, 48] as const;

/// The pixel size of each CSS absolute-size keyword.
const KEYWORD_SIZES: Record<string, number> = {
  "xx-small": 9,
  "x-small": 10,
  small: 13,
  medium: 16,
  large: 18,
  "x-large": 24,
  "xx-large": 32,
  "xxx-large": 48,
};

/// CSS pixels per unit for the absolute units mail carries. `em` and `rem` count against the
/// browser's 16px default, which is what an indent written in them is measured against.
const UNIT_PX: Record<string, number> = {
  px: 1,
  pt: 4 / 3,
  pc: 16,
  in: 96,
  cm: 96 / 2.54,
  mm: 96 / 25.4,
  em: 16,
  rem: 16,
};

/// A CSS length in pixels, or `null` for anything that is not one (a percentage, `auto`, a
/// keyword). `relative` decides whether `em` and `rem` count: they do for an indent, and not for a
/// font size, where they scale whatever the size around them is.
export function pxOfLength(value: string | null | undefined, relative = true): number | null {
  const match = /^(-?\d*\.?\d+)\s*([a-z]*)$/.exec((value ?? "").trim().toLowerCase());
  if (!match) return null;
  const unit = match[2] || "px";
  if (!relative && (unit === "em" || unit === "rem")) return null;
  const perUnit = UNIT_PX[unit];
  if (perUnit === undefined) return null;
  return Number.parseFloat(match[1]!) * perUnit;
}

/// The size a CSS `font-size` value stands for, or `null` when it is relative to its surroundings.
export function sizeOfCss(value: string | null | undefined): FontSize | null {
  const raw = (value ?? "").trim().toLowerCase();
  const keyword = KEYWORD_SIZES[raw];
  if (keyword !== undefined) return keyword;
  const px = pxOfLength(raw, false);
  return px === null ? null : fontSizeOf(px);
}

/// The size an HTML `<font size>` attribute stands for: 1 to 7, or a step from the default 3.
export function sizeOfLegacy(value: string | null | undefined): FontSize | null {
  const raw = (value ?? "").trim();
  const match = /^([+-]?)(\d)$/.exec(raw);
  if (!match) return null;
  const step = Number(match[2]);
  const level = match[1] === "+" ? 3 + step : match[1] === "-" ? 3 - step : step;
  return LEGACY_SIZES[Math.min(7, Math.max(1, level)) - 1]!;
}

/// The generic families CSS names without quotes. Mirrors `GENERIC_FAMILIES` in
/// `crates/mailcal-composer/src/font.rs`.
const GENERIC_FAMILIES = new Set(["serif", "sans-serif", "monospace", "cursive", "fantasy", "system-ui"]);

const MAX_NAMES = 8;
const MAX_NAME_LEN = 64;

/// A font-family list in the one spelling the document holds: names unquoted, joined by `", "`.
/// `null` for a list that names nothing, or names something no font is called. Mirrors
/// `FontFamily::new`, which refuses the same lists.
export function canonicalFamily(value: string | null | undefined): string | null {
  const names: string[] = [];
  for (const raw of (value ?? "").split(",")) {
    const name = raw.trim().replace(/^["']+|["']+$/g, "").trim();
    if (!name) continue;
    if ([...name].length > MAX_NAME_LEN || !/^[\p{L}\p{N}\s._-]+$/u.test(name)) return null;
    names.push(name.split(/\s+/).join(" "));
  }
  if (names.length === 0 || names.length > MAX_NAMES) return null;
  return names.join(", ");
}

/// A canonical family list as a CSS `font-family` value. Mirrors `FontFamily::css`.
export function familyCss(canonical: string): string {
  return canonical
    .split(", ")
    .map((name) => (GENERIC_FAMILIES.has(name.toLowerCase()) ? name : `'${name}'`))
    .join(", ");
}
