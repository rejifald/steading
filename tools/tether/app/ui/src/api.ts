import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** A repo's state, as `git tether status --json` spells it. */
export type Health = "in-sync" | "behind" | "skipped" | "needs-you";

export type Mode = "follow" | "mirror";

/** One row of `git tether status --json` (tether-core `report::Row`). */
export interface Row {
  name: string;
  /** As listed in repos.toml, with `~`. The key every command takes. */
  path: string;
  branch: string;
  mode: Mode;
  remote: string;
  interval_seconds: number;
  /** Shell command a background sync asks first (ADR 0004). */
  gate?: string;
  /** Shell command run after a sync moved the tip (ADR 0004). */
  after?: string;
  last_sync?: string;
  health: Health;
  reason?: { kind: string };
  /** What the owner does to clear `reason` (tether-core `Reason::hint`). */
  hint?: string;
  behind?: number;
}

/** `git tether status --json` (tether-core `report::Report`). */
export interface Report {
  paused: boolean;
  overall: Health;
  needs_you: number;
  repos: Row[];
}

/** What the app sends with every refresh: the report and what only the tray knows. */
export interface Snapshot {
  /** `null` before the first report, or when repos.toml cannot be read (see `error`). */
  report: Report | null;
  error: string | null;
  syncing: boolean;
  /** Paths the worker is syncing right now. */
  busy: string[];
  /** The last failed action (Pause, a sync), until the next refresh. */
  note: string | null;
  /** Per path, the CLI's own detail line: the skip reason or the commits to pull. */
  details: Record<string, string>;
}

export interface AddRequest {
  path: string;
  branch: string;
  mode: Mode;
  /** Clone URL; only a mirror at an empty path uses it. */
  url: string | null;
  /** `<n>s`, `<n>m` or `<n>h`; `null` keeps tether's default. */
  interval: string | null;
  /** Shell command a background sync asks first; `null` for none. */
  gate: string | null;
  /** Shell command run after a sync moved the tip; `null` for none. */
  after: string | null;
}

/** Everything the popover can ask of the app. Failing calls reject with the error text. */
export interface TetherApi {
  report(): Promise<Snapshot>;
  /** Calls `handler` after every refresh; resolves to an unsubscribe function. */
  onReport(handler: (snapshot: Snapshot) => void): Promise<() => void>;
  /** Queues a sync of one repo, or of all with `null`. */
  sync(path: string | null): Promise<void>;
  togglePause(): Promise<void>;
  /** The macOS folder picker; `null` when cancelled. */
  pickFolder(): Promise<string | null>;
  /** The branch a clone has checked out, for the add form's default. */
  branchOf(path: string): Promise<string | null>;
  add(request: AddRequest): Promise<void>;
  /** Rewrites a listed repo's poll interval (`<n>s`, `<n>m` or `<n>h`). */
  setInterval(path: string, interval: string): Promise<void>;
  /** Unlists a repo; its clone is left as it is. */
  remove(path: string): Promise<void>;
  openInFinder(path: string): Promise<void>;
  hide(): Promise<void>;
  quit(): Promise<void>;
}

const REPORT_EVENT = "report";

/** The command layer inside the app: one Tauri command per method. */
export const tauriApi: TetherApi = {
  report: () => invoke<Snapshot>("report"),
  onReport: (handler) => listen<Snapshot>(REPORT_EVENT, (event) => handler(event.payload)),
  sync: (path) => invoke("sync", { path }),
  togglePause: () => invoke("toggle_pause"),
  pickFolder: () => invoke<string | null>("pick_folder"),
  branchOf: (path) => invoke<string | null>("branch_of", { path }),
  add: (request) => invoke("add", { request }),
  setInterval: (path, interval) => invoke("set_interval", { path, interval }),
  remove: (path) => invoke("remove", { path }),
  openInFinder: (path) => invoke("open_in_finder", { path }),
  hide: () => invoke("hide"),
  quit: () => invoke("quit"),
};
