import type { Repo, State } from "./api";
import { type FakeOptions, sampleRepos } from "./api.fake";

/**
 * The popover's states (docs/design/popover/anatomy.md, round 2), for Storybook and for
 * `npm run dev`: open `/?scenario=<name>` to render one. Dev-only; the app never loads this file.
 */
const now = new Date();

/** The sample repos with only the worktrees in `states`. */
function only(states: readonly State[]): Repo[] {
  return sampleRepos(now).map((repo) => ({
    ...repo,
    worktrees: repo.worktrees.filter((worktree) => states.includes(worktree.state)),
  }));
}

const NOT_NEEDING_YOU: State[] = ["open", "removable", "missing", "dirty", "in-use", "locked", "kept"];

export const SCENARIOS: Record<string, FakeOptions> = {
  regular: {},
  "removal-pending": { repos: only(NOT_NEEDING_YOU) },
  tidy: { repos: only(["open", "dirty", "in-use", "locked", "kept"]) },
  refreshing: { repos: only(NOT_NEEDING_YOU), refreshing: true },
  removing: { repos: only(NOT_NEEDING_YOU), holdsRemovals: true },
  paused: { paused: true },
  "repo-error": {
    repos: [
      ...only(["removable"]),
      {
        name: "example-broken",
        path: "~/Development/example-broken",
        base: "origin/main",
        policy: "ask",
        error: "not a git repository (or any of the parent directories): .git",
        worktrees: [],
      },
    ],
  },
  "found-by-commit": {
    repos: [
      {
        name: "example-app",
        path: "~/Development/example-app",
        base: "origin/main",
        policy: "auto",
        worktrees: [
          {
            name: "review-fix-3f2a",
            path: "~/Development/example-app/.claude/worktrees/review-fix-3f2a",
            pr: { number: 212, state: "merged", url: "https://example.com/pull/212", by_commit: true },
            state: "removable",
            reason: "in #212, merged",
            last_activity: new Date(now.getTime() - 3 * 86_400_000).toISOString(),
            removes_at: new Date(now.getTime() + 5 * 3_600_000).toISOString(),
            size_bytes: 1_200_000_000,
          },
          {
            name: "issue-7",
            path: "~/Development/example-app/.claude/worktrees/issue-7",
            branch: "feat/issue-7",
            pr: { number: 300, state: "merged", url: "https://example.com/pull/300", by_commit: true },
            state: "removable",
            reason: "in #300, merged",
            last_activity: new Date(now.getTime() - 9 * 86_400_000).toISOString(),
            removes_at: new Date(now.getTime() + 20 * 3_600_000).toISOString(),
            size_bytes: 640_000_000,
          },
        ],
      },
    ],
  },
  notice: { note: "git worktree remove: exit status 128" },
  "stale-read-error": { repos: sampleRepos(now), error: "expected `=` at line 4" },
  loading: { isLoading: true },
  empty: { repos: [] },
  error: { error: "expected a table at line 3" },
};
