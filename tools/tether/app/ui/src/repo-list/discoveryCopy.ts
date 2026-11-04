import type { Discovery } from "../api";
import { agoInWords } from "./ago";

/** The words that join roots in a sentence: "A and B", "A or B", or "A, B" where a line is cramped. */
export type Joiner = "and" | "or" | "comma";

/**
 * Roots as they read in copy (anatomy § 5): one as is, two joined, three or more as the first
 * two and a count ("~/Development, ~/Code and 2 more").
 */
export function rootsText(roots: string[], joiner: Joiner = "and"): string {
  const word = joiner === "comma" ? "and" : joiner;
  const [first, second] = roots;
  if (first === undefined) return "";
  if (second === undefined) return first;
  if (roots.length === 2) return joiner === "comma" ? `${first}, ${second}` : `${first} ${word} ${second}`;
  const more = roots.length - 2;
  return `${first}, ${second} ${word} ${more} more`;
}

/** Where the last scan could look: the roots it read, in order. */
export const searchedOf = (discovery: Discovery): string[] =>
  discovery.roots.filter((root) => root.error === null).map((root) => root.path);

/** The root-level failures, one line each: what failed, then which roots were searched anyway. */
export function errorLinesOf(discovery: Discovery): string[] {
  const searched = searchedOf(discovery);
  const rest =
    searched.length === 0 ? "" : ` ${rootsText(searched)} ${searched.length === 1 ? "was" : "were"} searched.`;
  return discovery.roots.flatMap((root) =>
    root.error === null ? [] : [`Cannot read ${root.path}: ${root.error.replace(/\.$/, "")}.${rest}`],
  );
}

/**
 * The sentence under the open Found heading. A finished scan with an unreadable root has none
 * (panel 8): the error line already says which roots were searched.
 */
export function leadOf(discovery: Discovery, now: Date): string | undefined {
  if (discovery.scanning !== null) return "You can add these now; more appear as the search goes.";
  if (errorLinesOf(discovery).length > 0) return undefined;
  const searched = searchedOf(discovery);
  const where =
    searched.length === 0 || discovery.scanned_at === null
      ? ""
      : `In ${rootsText(searched)}, ${agoInWords(discovery.scanned_at, now)}. `;
  return `${where}Add the ones to keep on their branch.`;
}

/** The collapsed row's tail: "· looked 3 days ago"; none when the scan never finished. */
export function collapsedSummaryOf(discovery: Discovery, now: Date): string | undefined {
  return discovery.scanned_at === null ? undefined : `· looked ${agoInWords(discovery.scanned_at, now)}`;
}

/**
 * The settled row's text. "Nothing new in …" when the scan did find clones and they are all
 * listed or skipped; "No suggestions" when it found none.
 */
export function settledTextOf(discovery: Discovery, now: Date): string {
  const yielded = discovery.roots.filter((root) => root.error === null && (root.clones ?? 0) > 0);
  const what = yielded.length > 0 ? `Nothing new in ${rootsText(yielded.map((root) => root.path), "comma")}` : "No suggestions";
  return discovery.scanned_at === null ? what : `${what} · looked ${agoInWords(discovery.scanned_at, now)}`;
}

/** Where the popover puts discovery, from the data and from what this popover session has done. */
export type Placement = "invite" | "found-nothing" | "open" | "collapsed" | "settled" | "none";

/**
 * Anatomy § 6. With nothing listed the Found section is the body while a scan runs or
 * suggestions remain, `found-nothing` after a scan with none, the invitation before any scan.
 * With repos listed it sits under them: open while a scan runs or this session acted on
 * suggestions, collapsed while some remain, settled once none does, absent before any scan.
 */
export function placementOf(discovery: Discovery, hasRepos: boolean, hasActed: boolean): Placement {
  const isScanning = discovery.scanning !== null;
  const left = discovery.suggestions.length;
  if (!hasRepos) {
    if (isScanning || left > 0) return "open";
    return discovery.scanned_at === null ? "invite" : "found-nothing";
  }
  const hasScanned = discovery.scanned_at !== null || isScanning || left > 0;
  if (!hasScanned) return "none";
  if (isScanning) return "open";
  if (left === 0) return "settled";
  return hasActed ? "open" : "collapsed";
}
