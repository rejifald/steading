import type { Overall, ReapApi, Repo, Report, Snapshot, Worktree } from "./api";

/** How long a fake refresh takes, so the refreshing state is visible in a browser. */
const FAKE_REFRESH_TIME = 800;

const minutesAgo = (minutes: number, now: Date) =>
  new Date(now.getTime() - minutes * 60_000).toISOString();

/** Two repos covering every state, with the CLI's own reason lines. */
export function sampleRepos(now = new Date()): Repo[] {
  const worktree = (fields: Pick<Worktree, "name" | "state" | "reason"> & Partial<Worktree>): Worktree => ({
    path: `~/Development/worktrees/${fields.name}`,
    ...fields,
  });
  return [
    {
      name: "git-reap",
      path: "~/Development/git-reap",
      base: "origin/main",
      policy: "auto",
      worktrees: [
        worktree({
          name: "fix-login",
          branch: "fix/login",
          pr: { number: 41, state: "merged", url: "https://example.com/pull/41" },
          state: "removable",
          reason: "merged into origin/main",
          last_activity: minutesAgo(90, now),
          removes_at: minutesAgo(-20 * 60, now),
          size_bytes: 840_000_000,
        }),
        worktree({
          name: "spike-render",
          branch: "spike/render",
          state: "kept",
          reason: "merged into origin/main, but kept until it gets new commits",
          last_activity: minutesAgo(600, now),
        }),
        worktree({
          name: "old-build",
          branch: "build/old",
          state: "refused",
          reason: "merged into origin/main, but git refused to remove it: contains modified or untracked files",
          last_activity: minutesAgo(3000, now),
        }),
        worktree({
          name: "new-api",
          branch: "feat/api",
          state: "open",
          reason: "not merged into origin/main",
          last_activity: minutesAgo(5, now),
        }),
        worktree({
          name: "spike-cache",
          branch: "spike/cache",
          state: "dirty",
          reason: "has uncommitted changes",
          last_activity: minutesAgo(30, now),
          size_bytes: 3_240_000_000,
        }),
        worktree({
          name: "docs-pass",
          branch: "docs/pass",
          pr: { number: 57, state: "merged", url: "https://example.com/pull/57" },
          state: "in-use",
          reason: "a process is running inside it",
          last_activity: minutesAgo(2, now),
        }),
        worktree({
          name: "usb-build",
          branch: "build/usb",
          state: "locked",
          reason: "locked: on usb",
          last_activity: minutesAgo(600, now),
        }),
      ],
    },
    {
      name: "example-site",
      path: "~/Development/example-site",
      base: "origin/main",
      policy: "auto",
      gh_note: "gh is not installed: only ancestry merges are seen",
      worktrees: [
        worktree({
          name: "old-redesign",
          branch: "feat/redesign",
          pr: { number: 12, state: "closed", url: "https://example.com/pull/12" },
          state: "stale",
          reason: "pull request #12 closed without merging",
          last_activity: minutesAgo(60 * 24 * 12, now),
        }),
        worktree({
          name: "gone-fast",
          branch: "fix/typo",
          state: "missing",
          reason: "folder is gone",
        }),
        worktree({ name: "detached-probe", state: "open", reason: "not merged into origin/main" }),
      ],
    },
  ];
}

function overallOf(repos: Repo[]): Overall {
  const states = repos.flatMap((repo) => repo.worktrees.map((worktree) => worktree.state));
  if (states.some((state) => state === "stale" || state === "refused")) return "needs-you";
  if (states.some((state) => state === "removable" || state === "missing")) return "pending";
  return "tidy";
}

export interface FakeOptions {
  repos?: Repo[];
  /** The folder the fake picker returns; `null` acts as Cancel. */
  pickedFolder?: string | null;
  /** Starts paused. */
  paused?: boolean;
  /** Starts mid-refresh, and stays there: the refreshing state, held still for a story. */
  refreshing?: boolean;
  /** Remove now never finishes: the removing state, held still for a test or a story. */
  holdsRemovals?: boolean;
  /** repos.toml cannot be read: with no `repos`, the read-error panel; with them, the notice. */
  error?: string;
  /** The app's last failed action, shown in the notice strip. */
  note?: string;
  /** The first report never arrives: the skeleton. */
  isLoading?: boolean;
}

/** An in-memory app for tests and for `npm run dev` in a plain browser. */
export function createFakeApi(options: FakeOptions = {}): ReapApi {
  let repos = options.repos ?? sampleRepos();
  let paused = options.paused ?? false;
  let refreshing = options.refreshing ?? false;
  const pickedFolder =
    options.pickedFolder === undefined ? "~/Development/example-new" : options.pickedFolder;
  const handlers = new Set<(s: Snapshot) => void>();

  const snapshot = (): Snapshot => {
    const states = repos.flatMap((repo) => repo.worktrees.map((worktree) => worktree.state));
    const report: Report = {
      paused,
      overall: overallOf(repos),
      removable: states.filter((state) => state === "removable" || state === "missing").length,
      needs_you: states.filter((state) => state === "stale" || state === "refused").length,
      repos,
    };
    if (options.error !== undefined && options.repos === undefined) {
      return { report: null, error: options.error, refreshing, note: null };
    }
    return { report, error: options.error ?? null, refreshing, note: options.note ?? null };
  };
  const emit = () => handlers.forEach((handler) => handler(snapshot()));
  const setState = (path: string, state: Worktree["state"]) => {
    repos = repos.map((repo) => ({
      ...repo,
      worktrees: repo.worktrees.map((worktree) => (worktree.path === path ? { ...worktree, state } : worktree)),
    }));
    emit();
  };

  return {
    report: () => (options.isLoading ? new Promise<Snapshot>(() => {}) : Promise.resolve(snapshot())),
    onReport: async (handler) => {
      handlers.add(handler);
      return () => handlers.delete(handler);
    },
    refresh: async () => {
      refreshing = true;
      emit();
      setTimeout(() => {
        refreshing = false;
        emit();
      }, FAKE_REFRESH_TIME);
    },
    togglePause: async () => {
      paused = !paused;
      emit();
    },
    pickFolder: async () => pickedFolder,
    add: async (path) => {
      if (repos.some((repo) => repo.path === path)) {
        throw `${path} is already listed`;
      }
      const name = path.split("/").pop() ?? path;
      repos = [...repos, { name, path, base: "origin/main", policy: "auto", worktrees: [] }];
      emit();
    },
    openInFinder: async () => {},
    keep: async (path) => {
      setState(path, "kept");
    },
    unkeep: async (path) => {
      setState(path, "removable");
    },
    removeNow: async (path) => {
      // Like the core: only a removable (or git-refused, retried) worktree can be removed by hand.
      const target = repos.flatMap((repo) => repo.worktrees).find((worktree) => worktree.path === path);
      if (target && target.state !== "removable" && target.state !== "refused") {
        throw `${path} is not removable`;
      }
      if (options.holdsRemovals) return new Promise<void>(() => {});
      repos = repos.map((repo) => ({ ...repo, worktrees: repo.worktrees.filter((worktree) => worktree.path !== path) }));
      emit();
    },
    hide: async () => {},
    quit: async () => {},
  };
}
