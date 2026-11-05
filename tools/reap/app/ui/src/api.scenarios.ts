import type { Repo, RootInfo, SkippedClone, State } from "./api";
import { DEFAULT_ROOTS, type FakeOptions, sampleRepos, sampleSuggestions } from "./api.fake";

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

const root = (path: string, fields: Partial<RootInfo> = {}): RootInfo => ({
  path,
  clones: null,
  stopped_early: false,
  error: null,
  ...fields,
});

/** The default roots after a scan that found the sample clones: 4 of them, plus clones with no worktree. */
const SCANNED_ROOTS = [root("~/Development", { clones: 5 }), root("~/Code", { clones: 3 })];

const SKIPPED: SkippedClone[] = [
  { path: "~/Development/scratch", name: "scratch", root: "~/Development" },
  { path: "~/Code/old-site", name: "old-site", root: "~/Code" },
];

/** One clone with worktrees, so a repo is listed under Found. */
const LISTED = only(["open", "removable", "kept"]).slice(0, 1);

/** Clone discovery (docs/design/discovery/anatomy.md, panels 10 and 11 and their reap equivalents). */
const DISCOVERY: Record<string, FakeOptions> = {
  // Panel 10: nothing listed, no scan yet. The roots-in-copy variants follow.
  invite: { repos: [] },
  "invite-one-root": { repos: [], roots: [root("~/Development")] },
  "invite-three-roots": {
    repos: [],
    roots: [...DEFAULT_ROOTS.map((path) => root(path)), root("~/Projects"), root("~/src")],
  },
  "invite-no-roots": { repos: [], roots: [] },
  // Panel 2 / 1 looking: partial rows, ghosts, the hairline.
  "found-looking": {
    repos: [],
    scanning: "~/Code",
    found: sampleSuggestions().slice(0, 2),
    roots: [root("~/Development", { clones: 5 }), root("~/Code")],
  },
  "found-looking-nothing-yet": { repos: [], scanning: "~/Development" },
  // Panel 11 / 3: the whole body.
  found: { repos: [], found: sampleSuggestions(), scannedMinutesAgo: 0, roots: SCANNED_ROOTS, skipped: SKIPPED },
  "found-one": {
    repos: [],
    found: sampleSuggestions().slice(0, 1),
    scannedMinutesAgo: 0,
    roots: [root("~/Development", { clones: 2 })],
  },
  // Panel 8: a root that could not be read.
  "found-root-error": {
    repos: [],
    found: sampleSuggestions().slice(0, 2),
    scannedMinutesAgo: 0,
    roots: [
      root("~/Development", { clones: 5 }),
      root("~/Code", { clones: 3 }),
      root("~/work", { error: "permission denied" }),
    ],
  },
  // Panel 7: nothing found, nothing listed.
  "found-nothing": { repos: [], found: [], scannedMinutesAgo: 0, roots: [root("~/Development", { clones: 0 }), root("~/Code", { clones: 0 })] },
  "found-nothing-stopped-early": {
    repos: [],
    found: [],
    scannedMinutesAgo: 0,
    roots: [root("~/Development", { clones: 0 }), root("~/Code", { clones: 0, stopped_early: true })],
  },
  // Panel 4 / 5 / 6: listed repos with Found last.
  "found-listed": { repos: LISTED, found: sampleSuggestions(), scannedMinutesAgo: 3 * 24 * 60, roots: SCANNED_ROOTS, skipped: SKIPPED },
  "found-listed-scanning": {
    repos: LISTED,
    scanning: "~/Code",
    found: sampleSuggestions().slice(0, 2),
    roots: [root("~/Development", { clones: 5 }), root("~/Code")],
  },
  "found-listed-nothing-new": { repos: LISTED, found: [], scannedMinutesAgo: 0, roots: SCANNED_ROOTS, skipped: SKIPPED },
  "found-listed-no-suggestions": {
    repos: LISTED,
    found: [],
    scannedMinutesAgo: 0,
    roots: [root("~/Development", { clones: 0 }), root("~/Code", { clones: 0 })],
  },
  "found-listed-error": {
    repos: LISTED,
    found: sampleSuggestions().slice(0, 2),
    scannedMinutesAgo: 0,
    roots: [root("~/Development", { clones: 5 }), root("~/work", { error: "permission denied" })],
  },
  // A failed Add (the notice shows it, the row stays) and one in flight.
  "found-add-fails": {
    repos: [],
    found: sampleSuggestions(),
    scannedMinutesAgo: 0,
    roots: SCANNED_ROOTS,
    addError: "~/Development/api is a linked worktree; add the clone it belongs to",
  },
  "found-adding": { repos: [], found: sampleSuggestions(), scannedMinutesAgo: 0, roots: SCANNED_ROOTS, holdsAdds: true },
  // Panel 9 and its states: where to look.
  "where-to-look-no-skipped": { repos: [], found: sampleSuggestions(), scannedMinutesAgo: 0, roots: SCANNED_ROOTS },
  "where-to-look-error": {
    repos: [],
    found: sampleSuggestions().slice(0, 2),
    scannedMinutesAgo: 0,
    roots: [root("~/Development", { clones: 5 }), root("~/work", { error: "permission denied" })],
    skipped: SKIPPED,
  },
  "where-to-look-not-scanned": { repos: [], roots: DEFAULT_ROOTS.map((path) => root(path)), skipped: SKIPPED },
};

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
  ...DISCOVERY,
};
