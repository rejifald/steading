import { useCallback, useEffect, useState } from "react";

import { NoticeStrip } from "@steading/tray-popover/NoticeStrip";
import { PopoverFrame } from "@steading/tray-popover/PopoverFrame";
import { ReadError } from "@steading/tray-popover/ReadError";

import { AddForm } from "./add-form/AddForm";
import type { ReapApi, Snapshot, WorktreeAction } from "./api";
import { FoundArea } from "./discovery/FoundArea";
import { type FoundMode, foundModeOf } from "./discovery/foundMode";
import { RootsView } from "./discovery/RootsView";
import { Footer } from "./Footer";
import { Header } from "./Header";
import { usePopoverShown } from "./usePopoverShown";
import { EmptyState } from "./worktree-list/EmptyState";
import { ListScroll } from "./worktree-list/ListScroll";
import { LoadingRows } from "./worktree-list/LoadingRows";
import { WorktreeList } from "./worktree-list/WorktreeList";

/** Each row action as the command it runs. */
const ACTIONS: Record<WorktreeAction, (api: ReapApi, path: string) => Promise<void>> = {
  open: (api, path) => api.openInFinder(path),
  keep: (api, path) => api.keep(path),
  unkeep: (api, path) => api.unkeep(path),
  "remove-now": (api, path) => api.removeNow(path),
};

const NONE_REMOVING: ReadonlySet<string> = new Set();
const NONE_ADDING: ReadonlySet<string> = new Set();

/** What the popover shows: the list, the add form, or the folders a scan walks. */
export type View = { kind: "list" } | { kind: "add" } | { kind: "roots" };

const LIST: View = { kind: "list" };
const ADD: View = { kind: "add" };
const ROOTS: View = { kind: "roots" };

/** How often "3 min ago" is recomputed while the popover is open. */
const CLOCK_TICK = 30_000;

/** The whole tray window: header, the worktree list, the add form or where to look, footer. */
export function Popover({ api }: { api: ReapApi }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [view, setView] = useState<View>(LIST);
  const [actionError, setActionError] = useState<string | null>(null);
  /** Worktrees whose removal was clicked, until the next report says what happened. */
  const [removing, setRemoving] = useState<ReadonlySet<string>>(NONE_REMOVING);
  /** Suggestions whose *Add* was clicked, until a snapshot no longer lists them as suggestions. */
  const [adding, setAdding] = useState<ReadonlySet<string>>(NONE_ADDING);
  /** This popover session started a scan or answered a suggestion: Found stays open under the list. */
  const [isEngaged, setEngaged] = useState(false);
  const now = useNow(CLOCK_TICK);
  usePopoverShown(() => setEngaged(false));

  const receive = useCallback((next: Snapshot) => {
    setSnapshot(next);
    // A refresh starts with a snapshot that still lists the worktree being removed, so a mark
    // stays until a report no longer lists its worktree.
    setRemoving((paths) => {
      if (!next.report || paths.size === 0) return paths;
      const present = new Set(next.report.repos.flatMap((repo) => repo.worktrees.map((worktree) => worktree.path)));
      const kept = [...paths].filter((path) => present.has(path));
      return kept.length === paths.size ? paths : new Set(kept);
    });
    // The same for an add: the row is busy until the clone leaves Found.
    setAdding((paths) => {
      if (paths.size === 0) return paths;
      const present = new Set(next.discovery.suggestions.map((suggestion) => suggestion.path));
      const kept = [...paths].filter((path) => present.has(path));
      return kept.length === paths.size ? paths : new Set(kept);
    });
    // A scan seen running means the operator started it, here or from the CLI: keep Found open.
    if (next.discovery.scanning !== null) setEngaged(true);
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let isLive = true;
    void api.onReport(receive).then((stop) => (isLive ? (unlisten = stop) : stop()));
    api.report().then(receive, (e: unknown) => setActionError(String(e)));
    return () => {
      isLive = false;
      unlisten?.();
    };
  }, [api, receive]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") void api.hide();
      // An accessory app has no menu bar, so Cmd+Q needs wiring here.
      if (event.key === "q" && event.metaKey) void api.quit();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [api]);

  /** Runs one command; resolves to whether it succeeded, so a batch can stop at a failure. */
  const run = useCallback(async (action: Promise<void>): Promise<boolean> => {
    setActionError(null);
    try {
      await action;
      return true;
    } catch (e) {
      setActionError(String(e));
      return false;
    }
  }, []);

  const act = useCallback(
    async (action: WorktreeAction, path: string): Promise<boolean> => {
      if (action !== "remove-now") return run(ACTIONS[action](api, path));
      setRemoving((paths) => new Set(paths).add(path));
      const isDone = await run(ACTIONS[action](api, path));
      if (!isDone) {
        setRemoving((paths) => {
          const rest = new Set(paths);
          rest.delete(path);
          return rest;
        });
      }
      return isDone;
    },
    [api, run],
  );

  const find = useCallback(() => {
    setEngaged(true);
    return run(api.findClones());
  }, [api, run]);

  const addSuggestion = useCallback(
    async (path: string) => {
      setEngaged(true);
      setAdding((paths) => new Set(paths).add(path));
      const isDone = await run(api.addSuggestion(path));
      // A failed add leaves the row where it was, enabled; the notice says why.
      if (!isDone) {
        setAdding((paths) => {
          const rest = new Set(paths);
          rest.delete(path);
          return rest;
        });
      }
    },
    [api, run],
  );

  const skipClone = useCallback(
    (path: string) => {
      setEngaged(true);
      return run(api.skipClone(path));
    },
    [api, run],
  );

  /** *Add folder…*: the macOS picker, then the folder joins the roots. */
  const addRoot = useCallback(async () => {
    setActionError(null);
    try {
      const picked = await api.pickFolder();
      if (picked !== null) await api.addScanRoot(picked);
    } catch (e) {
      setActionError(String(e));
    }
  }, [api]);

  const report = snapshot?.report ?? null;
  const readError = snapshot && !report ? snapshot.error : null;
  const staleError = readError ? null : (snapshot?.error ?? null);
  const notice =
    actionError ??
    snapshot?.note ??
    (staleError ? `Cannot read repos.toml: ${staleError}. Showing the last report.` : null);
  const isEmptyList = report !== null && report.repos.length === 0;
  const isFoundShown =
    snapshot !== null && foundModeOf({ discovery: snapshot.discovery, hasRepos: false, isEngaged }) !== null;
  const foundTotal = snapshot?.discovery.suggestions.length ?? 0;
  const isScanning = (snapshot?.discovery.scanning ?? null) !== null;
  const retry = () => api.report().then(receive, (e: unknown) => setActionError(String(e)));

  const body = (() => {
    if (view.kind === "add") {
      return <AddForm api={api} onDone={() => setView(LIST)} onCancel={() => setView(LIST)} />;
    }
    if (snapshot === null) {
      return (
        <>
          <LoadingRows />
          <Footer
            hasRepos={false}
            isLoading
            paused={false}
            onRefresh={() => run(api.refresh())}
            onTogglePause={() => run(api.togglePause())}
            onAdd={() => setView(ADD)}
          />
        </>
      );
    }
    if (readError) {
      return (
        <ReadError
          title="Cannot read repos.toml"
          message={readError}
          hint="Fix the file, then try again. Nothing is checked until it reads."
          onRetry={() => void retry()}
        />
      );
    }
    if (view.kind === "roots") {
      return (
        <RootsView
          discovery={snapshot.discovery}
          onAddRoot={() => void addRoot()}
          onRemoveRoot={(path) => void run(api.removeScanRoot(path))}
          onUnskip={(path) => void run(api.unskipClone(path))}
          onDone={() => setView(LIST)}
          onLookAgain={() => {
            setView(LIST);
            void find();
          }}
        />
      );
    }
    const discovery = snapshot.discovery;
    const repos = report?.repos ?? [];
    const hasRepos = repos.length > 0;
    const foundMode = foundModeOf({ discovery, hasRepos, isEngaged });
    const found = (mode: FoundMode, hasRuleAbove: boolean) => (
      <FoundArea
        discovery={discovery}
        mode={mode}
        now={now}
        hasRuleAbove={hasRuleAbove}
        adding={adding}
        onAdd={(path) => void addSuggestion(path)}
        onSkip={(path) => void skipClone(path)}
        onLookAgain={() => void find()}
        onWhereToLook={() => setView(ROOTS)}
      />
    );
    const footer = (
      <Footer
        hasRepos={hasRepos}
        hasFound={!hasRepos && foundMode === "open"}
        isRefreshing={snapshot.refreshing}
        paused={report?.paused ?? false}
        onRefresh={() => run(api.refresh())}
        onTogglePause={() => run(api.togglePause())}
        onAdd={() => setView(ADD)}
      />
    );
    if (!hasRepos) {
      const emptyProps = { discovery, now, onAdd: () => setView(ADD), onWhereToLook: () => setView(ROOTS) };
      return (
        <>
          {foundMode === "open" ? (
            <ListScroll>{found("open", false)}</ListScroll>
          ) : discovery.scanned_at === null ? (
            <EmptyState variant="invite" onFind={() => void find()} {...emptyProps} />
          ) : (
            <EmptyState variant="found-nothing" {...emptyProps} />
          )}
          {footer}
        </>
      );
    }
    return (
      <>
        <WorktreeList
          repos={repos}
          now={now}
          onAction={act}
          removing={removing}
          found={foundMode === null ? null : found(foundMode, true)}
        />
        {footer}
      </>
    );
  })();

  return (
    <PopoverFrame>
        <Header
          snapshot={snapshot}
          isPending={
            view.kind === "list" &&
            ((snapshot?.refreshing ?? false) ||
              removing.size > 0 ||
              adding.size > 0 ||
              isScanning)
          }
          foundCount={view.kind === "list" && isEmptyList && isFoundShown ? foundTotal : null}
          {...(view.kind === "add" ? { title: "Add repository", showState: false } : {})}
          {...(view.kind === "roots" ? { title: "Where to look", showState: false } : {})}
          {...(view.kind === "list" && isEmptyList ? { showState: false } : {})}
        />
        <NoticeStrip text={notice} />
        {body}
    </PopoverFrame>
  );
}

function useNow(every: number): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), every);
    return () => clearInterval(id);
  }, [every]);
  return now;
}
