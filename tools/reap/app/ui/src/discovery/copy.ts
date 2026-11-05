import type { Discovery, RootInfo, Suggestion } from "../api";
import { lookedAgo } from "../worktree-list/ago";

/** The folders a scan walks, in the words of a sentence: one, two, or the first two and "N more". */
export function rootsText(roots: readonly string[]): string {
  const [first, second] = roots;
  if (first === undefined) return "";
  if (second === undefined) return first;
  if (roots.length === 2) return `${first} and ${second}`;
  return `${first}, ${second} and ${roots.length - 2} more`;
}

export const pathsOf = (roots: readonly RootInfo[]): string[] => roots.map((root) => root.path);

/** The roots a scan could read: the ones it searched. */
const searchedOf = (roots: readonly RootInfo[]): string[] =>
  roots.filter((root) => root.error === null).map((root) => root.path);

/** "7 worktrees", "1 worktree". */
export function worktreesText(count: number): string {
  return `${count} ${count === 1 ? "worktree" : "worktrees"}`;
}

/** A row's faint detail: "7 worktrees · ~/Development". */
export const detailOf = (suggestion: Suggestion): string =>
  `${worktreesText(suggestion.worktrees)} · ${suggestion.root}`;

/** "5 clones", "1 clone": what a root yielded. */
export function clonesText(count: number): string {
  return `${count} ${count === 1 ? "clone" : "clones"}`;
}

/** The sentence under the Found heading: what to do while a scan runs, what was searched once it is done. */
export function leadOf(discovery: Discovery, now: Date): string {
  if (discovery.scanning !== null) return "You can add these now; more appear as the search goes.";
  const searched = searchedOf(discovery.roots);
  const where = searched.length > 0 ? ` in ${rootsText(searched)}` : "";
  const when = discovery.scanned_at ? `, ${lookedAgo(discovery.scanned_at, now)}` : "";
  return `Clones with worktrees${where}${when}. Merged worktrees go 24 h after reap first sees them.`;
}

/** One line per root the last scan could not read, naming the roots that were searched. */
export function errorLinesOf(roots: readonly RootInfo[]): string[] {
  const searched = searchedOf(roots);
  return roots
    .filter((root): root is RootInfo & { error: string } => root.error !== null)
    .map((root) => {
      const others =
        searched.length === 0 ? "" : ` ${rootsText(searched)} ${searched.length === 1 ? "was" : "were"} searched.`;
      return `Cannot read ${root.path}: ${root.error}.${others}`;
    });
}

/** The settled row: nothing left to answer, and whether the roots had clones at all. */
export function settledTextOf(discovery: Discovery, now: Date): string {
  const when = `looked ${lookedAgo(discovery.scanned_at, now)}`;
  const yielded = discovery.roots.some((root) => (root.clones ?? 0) > 0);
  return yielded ? `Nothing new in ${rootsText(pathsOf(discovery.roots))} · ${when}` : `No suggestions · ${when}`;
}

/** The faint line under the found-nothing empty state. */
export function lookedLineOf(discovery: Discovery, now: Date): string {
  const when = `Looked ${lookedAgo(discovery.scanned_at, now)}`;
  const stopped = discovery.roots.filter((root) => root.stopped_early).map((root) => root.path);
  return stopped.length > 0 ? `${when} · stopped early in ${rootsText(stopped)} after 20 000 folders.` : `${when}.`;
}
