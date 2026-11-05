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

/** One folder a scan walks, joined with what the last scan found in it (`discovery::RootInfo`). */
export interface RootInfo {
  /** As configured, with `~`. */
  path: string;
  /** Main clones the walk found there; `null` when the last scan did not cover it. */
  clones: number | null;
  /** The walk hit its folder or time cap before it had seen every folder. */
  stopped_early: boolean;
  /** Why the folder could not be read; `null` when it could. */
  error: string | null;
}

/** A clone the operator skipped, listed in *Where to look* so it can be unskipped. */
export interface SkippedClone {
  path: string;
  name: string;
  /** The longest scan root the path lies under; `null` when none does. */
  root: string | null;
}

/** A clone a scan found that tether would keep on a branch (`discovery::Suggestion`). */
export interface Suggestion {
  /** As it would be listed, with `~`. The key every discovery command takes. */
  path: string;
  name: string;
  /** The scan root it was found under, with `~`. */
  root: string;
  /** The branch *Add* would follow. */
  branch: string;
}

/** What `discovery.json` holds, minus the clones already listed or skipped (`discovery::View`). */
export interface Discovery {
  /** When the last scan ended; `null` before any scan, or while the first one runs. */
  scanned_at: string | null;
  /** The root being walked right now; `null` when no scan runs. */
  scanning: string | null;
  roots: RootInfo[];
  suggestions: Suggestion[];
  skipped: SkippedClone[];
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
  /** The clones the last scan found, and where it looks (ADR 0004). */
  discovery: Discovery;
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
  /** Starts a scan of the scan roots and returns at once; the results arrive with the refreshes. */
  findClones(): Promise<void>;
  /** Lists a suggestion as *follow* on its branch. Rejects with `add`'s own error text. */
  addSuggestion(path: string): Promise<void>;
  /** Hides a clone from the suggestions until it is unskipped. */
  skipClone(path: string): Promise<void>;
  unskipClone(path: string): Promise<void>;
  /** Adds a folder to the scan roots; nothing is scanned until *Look again*. */
  addScanRoot(path: string): Promise<void>;
  /** Drops a folder from the scan roots, and its suggestions with it. */
  removeScanRoot(path: string): Promise<void>;
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
  findClones: () => invoke("find_clones"),
  addSuggestion: (path) => invoke("add_suggestion", { path }),
  skipClone: (path) => invoke("skip_clone", { path }),
  unskipClone: (path) => invoke("unskip_clone", { path }),
  addScanRoot: (path) => invoke("add_scan_root", { path }),
  removeScanRoot: (path) => invoke("remove_scan_root", { path }),
  hide: () => invoke("hide"),
  quit: () => invoke("quit"),
};
