import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** What reap decided about one linked worktree, as the CLI spells it. */
export type State =
  | "open"
  | "removable"
  | "dirty"
  | "in-use"
  | "locked"
  | "stale"
  | "abandoned"
  | "missing"
  | "kept"
  | "refused";

/** What the popover can do to one worktree. */
export type WorktreeAction = "open" | "keep" | "unkeep" | "remove-now" | "save-and-remove";

/** The whole tray's state: nothing to do, a removal pending, or something only the user can fix. */
export type Overall = "tidy" | "pending" | "needs-you";

export interface PullRequest {
  number: number;
  state: "open" | "merged" | "closed";
  url: string;
  /** Found by HEAD's commit rather than by the branch: `in #212, merged` (ADR 0006). */
  by_commit?: boolean;
}

export interface Worktree {
  /** The folder name. */
  name: string;
  /** With `~`; the key commands take. */
  path: string;
  /** Short branch name; absent when HEAD is detached. */
  branch?: string;
  /** Full sha. */
  head?: string;
  pr?: PullRequest;
  state: State;
  /** The CLI's own detail line, always present. */
  reason: string;
  /** RFC 3339. */
  last_activity?: string;
  /** When a sweep removes it, RFC 3339; only once its grace clock runs, under policy "auto". */
  removes_at?: string;
  /** When its uncommitted changes, unchanged by then, are saved to a ref and it becomes removable. */
  saves_at?: string;
  /** The disk space removing it would free; absent when the folder is gone or unreadable. */
  size_bytes?: number;
  /** A stale worktree's work that exists only here, each as `reason` names it (ADR 0006 rule 9). */
  local?: string[];
  /** The guard that holds a stale worktree; also the end of `reason`. */
  held_by?: string;
}

/** A removed worktree's state saved to a ref under `refs/reap/salvage/`. */
export interface Salvaged {
  refname: string;
  commit: string;
  /** A merged worktree's changes expire; an abandoned worktree's save stays until deleted. */
  kind: "merged" | "abandoned";
  /** An abandoned save's `Reap-Reason`: `abandoned (<stale reason>)`. */
  reason?: string;
  /** The worktree it was saved from, with `~`. */
  worktree?: string;
  branch?: string;
  /** RFC 3339. */
  saved_at: string;
  /** When a sweep deletes the ref, RFC 3339; under policy "auto" only. */
  prunes_at?: string;
  /** The shell commands that bring the changes back, uncommitted, in a new worktree. */
  restore: string;
  /** The command that deletes the save. */
  delete: string;
}

export interface Repo {
  name: string;
  /** With `~`. */
  path: string;
  /** The ref worktrees are compared with, e.g. `origin/main`. */
  base: string;
  /** What reap does with this repo's removable worktrees. */
  policy: "auto" | "ask" | "off";
  /** Why `gh` could not be used. */
  gh_note?: string;
  /** The repo could not be read at all; `worktrees` is then empty. */
  error?: string;
  /** Linked worktrees only, never the main clone. */
  worktrees: Worktree[];
  /** Saves of removed worktrees' changes, oldest first. */
  salvaged?: Salvaged[];
}

export interface Report {
  paused: boolean;
  overall: Overall;
  removable: number;
  needs_you: number;
  repos: Repo[];
}

/** What the app sends with every refresh: the report and what only the tray knows. */
export interface Snapshot {
  /** `null` before the first report, or when repos.toml cannot be read (see `error`). */
  report: Report | null;
  error: string | null;
  refreshing: boolean;
  /** The last failed action, until the next refresh. */
  note: string | null;
}

/** Everything the popover can ask of the app. Failing calls reject with the error text. */
export interface ReapApi {
  report(): Promise<Snapshot>;
  /** Calls `handler` after every refresh; resolves to an unsubscribe function. */
  onReport(handler: (snapshot: Snapshot) => void): Promise<() => void>;
  refresh(): Promise<void>;
  togglePause(): Promise<void>;
  /** The macOS folder picker; `null` when cancelled. */
  pickFolder(): Promise<string | null>;
  add(path: string): Promise<void>;
  openInFinder(path: string): Promise<void>;
  /** Holds a worktree until it gets new commits. */
  keep(path: string): Promise<void>;
  unkeep(path: string): Promise<void>;
  /** Removes a removable worktree at once, with the same guards and no grace period. */
  removeNow(path: string): Promise<void>;
  /** Saves an abandoned worktree to a ref, then removes it, keeping its branch. */
  saveAndRemove(path: string): Promise<void>;
  /** Deletes one save, in the repo it is listed under. */
  deleteSalvaged(repo: string, refname: string): Promise<void>;
  hide(): Promise<void>;
  quit(): Promise<void>;
}

const REPORT_EVENT = "report";

/** The command layer inside the app: one Tauri command per method. */
export const tauriApi: ReapApi = {
  report: () => invoke<Snapshot>("report"),
  onReport: (handler) => listen<Snapshot>(REPORT_EVENT, (event) => handler(event.payload)),
  refresh: () => invoke("refresh"),
  togglePause: () => invoke("toggle_pause"),
  pickFolder: () => invoke<string | null>("pick_folder"),
  add: (path) => invoke("add", { path }),
  openInFinder: (path) => invoke("open_in_finder", { path }),
  keep: (path) => invoke("keep", { path }),
  unkeep: (path) => invoke("unkeep", { path }),
  removeNow: (path) => invoke("remove_now", { path }),
  saveAndRemove: (path) => invoke("save_and_remove", { path }),
  deleteSalvaged: (repo, refname) => invoke("delete_salvaged", { repo, refname }),
  hide: () => invoke("hide"),
  quit: () => invoke("quit"),
};
