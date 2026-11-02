import type { AddRequest, Report, Row, Snapshot, TetherApi } from "./api";
import { DEFAULT_INTERVAL_SECONDS } from "./interval/intervals";

/** How long a fake sync takes, so the busy state is visible in a browser. */
const FAKE_SYNC_TIME = 800;

export const minutesAgo = (minutes: number, now: Date) =>
  new Date(now.getTime() - minutes * 60_000).toISOString();

export const row = (fields: Partial<Row> & Pick<Row, "name" | "path">): Row => ({
  branch: "main",
  mode: "follow",
  remote: "origin",
  interval_seconds: 300,
  health: "in-sync",
  ...fields,
});

/** One repo in every state, with the CLI's own detail lines. */
export function sampleRows(now = new Date()): { rows: Row[]; details: Record<string, string> } {
  const rows = [
    row({ name: "example-app", path: "~/Development/example-app", last_sync: minutesAgo(2, now) }),
    row({
      name: "example-api",
      path: "~/Development/example-api",
      health: "behind",
      behind: 3,
      last_sync: minutesAgo(4, now),
    }),
    row({
      name: "example-docs",
      path: "~/Mirrors/example-docs",
      mode: "mirror",
      health: "skipped",
      reason: { kind: "busy" },
      last_sync: minutesAgo(12, now),
    }),
    row({
      name: "example-site",
      path: "~/Development/example-site",
      branch: "release",
      health: "needs-you",
      reason: { kind: "other-branch" },
      hint: "Switch back to release and tether resumes.",
      last_sync: minutesAgo(65, now),
    }),
  ];
  const details: Record<string, string> = {
    "~/Development/example-api": "3 commit(s) to pull on the next sync",
    "~/Mirrors/example-docs": "another tether run is syncing it; retrying next round",
    "~/Development/example-site": "on another branch (feature/new-header)",
  };
  return { rows, details };
}

function overall(rows: Row[]): Report["overall"] {
  for (const health of ["needs-you", "skipped", "behind"] as const) {
    if (rows.some((r) => r.health === health)) return health;
  }
  return "in-sync";
}

export interface FakeOptions {
  rows?: Row[];
  details?: Record<string, string>;
  /** The folder the fake picker returns; `null` acts as Cancel. */
  pickedFolder?: string | null;
  /** What the fake reads as the folder's checked-out branch; `null` when it cannot tell. */
  checkedOutBranch?: string | null;
  paused?: boolean;
  /** Paths already syncing when the popover opens. */
  busy?: string[];
  /** repos.toml cannot be read; with `hasReport`, the last report is still shown. */
  error?: string;
  hasReport?: boolean;
  /** The app's last failed action. */
  note?: string;
  /** The first report never arrives: the loading state. */
  isLoading?: boolean;
  /** Add, save and remove never finish: the pending states. */
  pendingActions?: boolean;
  /** Add, save and remove fail with this text: the error callouts. */
  rejectWith?: string;
}

/** An in-memory app for tests and for `npm run dev` in a plain browser. */
export function createFakeApi(options: FakeOptions = {}): TetherApi {
  const sample = sampleRows();
  let rows = options.rows ?? sample.rows;
  let details = options.details ?? (options.rows ? {} : sample.details);
  let paused = options.paused ?? false;
  let busy: string[] = options.busy ?? [];
  const error = options.error ?? null;
  let note = options.note ?? null;
  const pickedFolder =
    options.pickedFolder === undefined ? "~/Development/example-new" : options.pickedFolder;
  const handlers = new Set<(s: Snapshot) => void>();

  const snapshot = (): Snapshot => ({
    report:
      error && !options.hasReport
        ? null
        : {
            paused,
            overall: overall(rows),
            needs_you: rows.filter((r) => r.health === "needs-you").length,
            repos: rows,
          },
    error,
    syncing: busy.length > 0,
    busy,
    note,
    details,
  });
  const emit = () => handlers.forEach((h) => h(snapshot()));
  /** Runs a form's action, unless the options hold it pending or refuse it. */
  const act = (action: () => void): Promise<void> => {
    if (options.pendingActions) return new Promise(() => {});
    if (options.rejectWith) return Promise.reject(options.rejectWith);
    try {
      action();
      return Promise.resolve();
    } catch (e) {
      return Promise.reject(e);
    }
  };

  return {
    report: () => (options.isLoading ? new Promise<Snapshot>(() => {}) : Promise.resolve(snapshot())),
    onReport: async (handler) => {
      // One entry per call, as Tauri's `listen` gives: StrictMode subscribes the same
      // handler twice, and the first unsubscribe must not end the second.
      const subscription = (s: Snapshot) => handler(s);
      handlers.add(subscription);
      return () => handlers.delete(subscription);
    },
    sync: async (path) => {
      busy = rows.map((r) => r.path).filter((p) => path === null || p === path);
      emit();
      setTimeout(() => {
        const now = new Date().toISOString();
        rows = rows.map((r) =>
          busy.includes(r.path) && r.health !== "needs-you"
            ? { ...r, health: "in-sync", last_sync: now }
            : r,
        );
        details = Object.fromEntries(
          Object.entries(details).filter(
            ([p]) => !busy.includes(p) || rows.find((r) => r.path === p)?.health === "needs-you",
          ),
        );
        busy = [];
        emit();
      }, FAKE_SYNC_TIME);
    },
    togglePause: async () => {
      paused = !paused;
      note = null;
      emit();
    },
    pickFolder: async () => pickedFolder,
    branchOf: async () => (options.checkedOutBranch === undefined ? "main" : options.checkedOutBranch),
    add: ({ path, branch, mode, interval, gate, after }: AddRequest) =>
      act(() => {
        if (rows.some((r) => r.path === path)) {
          throw `${path} is already listed; \`git tether remove\` it first to change it`;
        }
        const name = path.split("/").pop() ?? path;
        const interval_seconds = secondsOf(interval) ?? DEFAULT_INTERVAL_SECONDS;
        const commands = { ...(gate && { gate }), ...(after && { after }) };
        rows = [...rows, row({ name, path, branch, mode, interval_seconds, ...commands })];
        emit();
      }),
    setInterval: (path, interval) =>
      act(() => {
        const seconds = secondsOf(interval);
        if (seconds === null) throw `interval "${interval}": expected <n>s, <n>m or <n>h`;
        rows = rows.map((r) => (r.path === path ? { ...r, interval_seconds: seconds } : r));
        emit();
      }),
    remove: (path) =>
      act(() => {
        if (!rows.some((r) => r.path === path)) throw `${path} is no longer listed`;
        rows = rows.filter((r) => r.path !== path);
        details = Object.fromEntries(Object.entries(details).filter(([p]) => p !== path));
        emit();
      }),
    openInFinder: async () => {},
    hide: async () => {},
    quit: async () => {},
  };
}

const UNIT_SECONDS = { s: 1, m: 60, h: 3600 } as const;
/** tether-core's `MIN_INTERVAL`. */
const MIN_INTERVAL_SECONDS = 30;

/** `<n>s`, `<n>m` or `<n>h` in seconds, as tether-core parses it; `null` when it would refuse it. */
function secondsOf(interval: string | null): number | null {
  const match = interval === null ? null : /^(\d+)([smh])$/.exec(interval);
  if (!match) return null;
  const seconds = Number(match[1]) * UNIT_SECONDS[match[2] as keyof typeof UNIT_SECONDS];
  return seconds < MIN_INTERVAL_SECONDS ? null : seconds;
}
