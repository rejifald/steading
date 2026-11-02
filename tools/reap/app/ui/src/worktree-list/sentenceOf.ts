import type { Repo, Worktree } from "../api";
import { inTime } from "./ago";

const MISSING_SENTENCE = "Folder is already gone. A sweep clears git’s record.";
const STALE_SUFFIX = "reap won’t remove it.";

function sentenceCase(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function removableSentence(worktree: Worktree, repo: Pick<Repo, "policy">, now: Date): string {
  const base =
    worktree.pr?.state === "merged" ? `Merged in #${worktree.pr.number}` : sentenceCase(worktree.reason);
  if (worktree.removes_at) return `${base} · removes ${inTime(worktree.removes_at, now)}`;
  if (repo.policy === "off") return `${base} · auto-removal is off`;
  return base;
}

function staleSentence(worktree: Worktree): string {
  const reason = sentenceCase(worktree.reason);
  return `${reason.endsWith(".") ? reason : `${reason}.`} ${STALE_SUFFIX}`;
}

/** Line 2 of a row: what happens to the worktree, in a sentence. */
export function sentenceOf(worktree: Worktree, repo: Pick<Repo, "policy">, now: Date): string {
  switch (worktree.state) {
    case "removable":
      return removableSentence(worktree, repo, now);
    case "missing":
      return MISSING_SENTENCE;
    case "stale":
      return staleSentence(worktree);
    default:
      return sentenceCase(worktree.reason);
  }
}
