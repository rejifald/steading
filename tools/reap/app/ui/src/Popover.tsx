import { useCallback, useEffect, useState } from "react";

import { NoticeStrip } from "@steading/tray-popover/NoticeStrip";
import { PopoverFrame } from "@steading/tray-popover/PopoverFrame";
import { ReadError } from "@steading/tray-popover/ReadError";

import { AddForm } from "./add-form/AddForm";
import type { ReapApi, Snapshot, WorktreeAction } from "./api";
import { Footer } from "./Footer";
import { Header } from "./Header";
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

/** How often "3 min ago" is recomputed while the popover is open. */
const CLOCK_TICK = 30_000;

/** The whole tray window: header, the worktree list or the add form, footer. */
export function Popover({ api }: { api: ReapApi }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [view, setView] = useState<"list" | "add">("list");
  const [actionError, setActionError] = useState<string | null>(null);
  /** Worktrees whose removal was clicked, until the next report says what happened. */
  const [removing, setRemoving] = useState<ReadonlySet<string>>(NONE_REMOVING);
  const now = useNow(CLOCK_TICK);

  const receive = useCallback((next: Snapshot) => {
    setSnapshot(next);
    setRemoving(NONE_REMOVING);
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

  const report = snapshot?.report ?? null;
  const readError = snapshot && !report ? snapshot.error : null;
  const staleError = readError ? null : (snapshot?.error ?? null);
  const notice =
    actionError ??
    snapshot?.note ??
    (staleError ? `Cannot read repos.toml: ${staleError}. Showing the last report.` : null);
  const isEmptyList = report !== null && report.repos.length === 0;
  const retry = () => api.report().then(receive, (e: unknown) => setActionError(String(e)));

  const body = (() => {
    if (view === "add") {
      return <AddForm api={api} onDone={() => setView("list")} onCancel={() => setView("list")} />;
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
            onAdd={() => setView("add")}
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
    return (
      <>
        <WorktreeList
          repos={report?.repos ?? []}
          now={now}
          onAction={act}
          removing={removing}
          onAdd={() => setView("add")}
        />
        <Footer
          hasRepos={(report?.repos.length ?? 0) > 0}
          isRefreshing={snapshot.refreshing}
          paused={report?.paused ?? false}
          onRefresh={() => run(api.refresh())}
          onTogglePause={() => run(api.togglePause())}
          onAdd={() => setView("add")}
        />
      </>
    );
  })();

  return (
    <PopoverFrame>
        <Header
          snapshot={snapshot}
          isPending={view === "list" && ((snapshot?.refreshing ?? false) || removing.size > 0)}
          {...(view === "add" ? { title: "Add repository", showState: false } : {})}
          {...(view === "list" && isEmptyList ? { showState: false } : {})}
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
