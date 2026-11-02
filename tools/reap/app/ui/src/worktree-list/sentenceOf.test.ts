import { describe, expect, it } from "vitest";

import type { Repo, State, Worktree } from "../api";
import { sentenceOf } from "./sentenceOf";

const NOW = new Date("2026-10-29T12:00:00Z");
const ask: Pick<Repo, "policy"> = { policy: "ask" };

function worktree(state: State, fields: Partial<Worktree> = {}): Worktree {
  return { name: "folder", path: "~/w/folder", state, reason: "merged into origin/main", ...fields };
}

describe("sentenceOf", () => {
  it("names the merged pull request for a removable worktree", () => {
    const pr = { number: 403, state: "merged" as const, url: "https://example.com/pull/403" };
    expect(sentenceOf(worktree("removable", { pr }), ask, NOW)).toBe("Merged in #403");
  });

  it("falls back to the reason when the pull request is not merged", () => {
    const pr = { number: 9, state: "open" as const, url: "https://example.com/pull/9" };
    expect(sentenceOf(worktree("removable", { pr }), ask, NOW)).toBe("Merged into origin/main");
    expect(sentenceOf(worktree("removable"), ask, NOW)).toBe("Merged into origin/main");
  });

  it("adds when the sweep removes it", () => {
    const removesAt = new Date(NOW.getTime() + 3 * 3600_000).toISOString();
    expect(sentenceOf(worktree("removable", { removes_at: removesAt }), { policy: "auto" }, NOW)).toBe(
      "Merged into origin/main · removes in 3 h",
    );
  });

  it("says auto-removal is off under policy off", () => {
    expect(sentenceOf(worktree("removable"), { policy: "off" }, NOW)).toBe(
      "Merged into origin/main · auto-removal is off",
    );
  });

  it("explains that a missing folder is cleared from git's record", () => {
    expect(sentenceOf(worktree("missing", { reason: "folder is gone" }), ask, NOW)).toBe(
      "Folder is already gone. A sweep clears git’s record.",
    );
  });

  it("ends a stale reason with the sentence that reap leaves it alone", () => {
    const stale = worktree("stale", { reason: "pull request #35 closed without merging" });
    expect(sentenceOf(stale, ask, NOW)).toBe("Pull request #35 closed without merging. reap won’t remove it.");
  });

  it("does not double the full stop of a stale reason", () => {
    const stale = worktree("stale", { reason: "closed without merging." });
    expect(sentenceOf(stale, ask, NOW)).toBe("Closed without merging. reap won’t remove it.");
  });

  it.each<[State, string, string]>([
    ["dirty", "has uncommitted changes", "Has uncommitted changes"],
    ["in-use", "merged into origin/main, but a process is running inside it", "Merged into origin/main, but a process is running inside it"],
    ["locked", "locked: on usb", "Locked: on usb"],
    ["open", "not merged into origin/main", "Not merged into origin/main"],
    ["kept", "kept until it gets new commits", "Kept until it gets new commits"],
    ["refused", "git refused to remove it", "Git refused to remove it"],
  ])("sentence-cases the reason of a %s worktree", (state, reason, expected) => {
    expect(sentenceOf(worktree(state, { reason }), ask, NOW)).toBe(expected);
  });

  it("adds no removal suffix to states other than removable", () => {
    const removesAt = new Date(NOW.getTime() + 3600_000).toISOString();
    expect(sentenceOf(worktree("dirty", { reason: "dirty", removes_at: removesAt }), { policy: "off" }, NOW)).toBe("Dirty");
  });
});
