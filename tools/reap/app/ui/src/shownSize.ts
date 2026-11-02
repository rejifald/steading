import type { Worktree } from "./api";

const UNITS = ["bytes", "kB", "MB", "GB", "TB"];
const STEP = 1000;

/** Bytes in decimal units as Finder (and `git reap status`) shows them: "840 MB", "3.2 GB", "14 GB". */
export function shownSize(bytes: number): string {
  let value = bytes;
  let unit = 0;
  while (value >= STEP && unit < UNITS.length - 1) {
    value /= STEP;
    unit += 1;
  }
  const digits = unit === 0 || value >= 10 ? 0 : 1;
  return `${value.toFixed(digits)} ${UNITS[unit]}`;
}

/** The disk the worktrees take, counting only the measured ones; 0 when none was measured. */
export function totalSizeOf(worktrees: Pick<Worktree, "size_bytes">[]): number {
  return worktrees.reduce((total, worktree) => total + (worktree.size_bytes ?? 0), 0);
}
