import type { Repo, Worktree } from "../api";
import { inTime } from "./ago";

const MISSING_SENTENCE = "Folder is already gone. A sweep clears git’s record.";
const GUARD_SEPARATOR = " · ";

function sentenceCase(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function withStop(text: string): string {
  return text.endsWith(".") ? text : `${text}.`;
}

/** "Merged in #403" for the branch's own pull request, "In #212, merged" for one found by HEAD's commit. */
function mergedIn(pr: NonNullable<Worktree["pr"]>): string {
  return pr.by_commit ? `In #${pr.number}, merged` : `Merged in #${pr.number}`;
}

function removableSentence(worktree: Worktree, repo: Pick<Repo, "policy">, now: Date): string {
  const base = worktree.pr?.state === "merged" ? mergedIn(worktree.pr) : sentenceCase(worktree.reason);
  if (worktree.removes_at) return `${base} · removes ${inTime(worktree.removes_at, now)}`;
  if (repo.policy === "off") return `${base} · auto-removal is off`;
  return base;
}

/** What is local, then, only when a guard holds it, that reap won’t remove it and which guard (ADR 0006 rule 9). */
function staleSentence(worktree: Worktree): string {
  const guard = worktree.held_by;
  const suffix = guard ? `${GUARD_SEPARATOR}${guard}` : null;
  if (!guard || !suffix || !worktree.reason.endsWith(suffix)) return withStop(sentenceCase(worktree.reason));
  const local = worktree.reason.slice(0, -suffix.length);
  return `${withStop(sentenceCase(local))} reap won’t remove it: ${withStop(guard)}`;
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
