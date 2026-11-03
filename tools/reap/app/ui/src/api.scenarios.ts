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
  notice: { note: "git worktree remove: exit status 128" },
  "stale-read-error": { repos: sampleRepos(now), error: "expected `=` at line 4" },
  loading: { isLoading: true },
  empty: { repos: [] },
  error: { error: "expected a table at line 3" },
};
