import { useCallback, useEffect, useState } from "react";

import { NoticeStrip } from "@steading/tray-popover/NoticeStrip";
import { PopoverFrame } from "@steading/tray-popover/PopoverFrame";
import { ReadError } from "@steading/tray-popover/ReadError";

import { AddForm } from "./add-form/AddForm";
import type { Snapshot, TetherApi } from "./api";
import { Footer } from "./Footer";
import { Header } from "./Header";
import { noticeOf } from "./noticeOf";
import { LoadingRows } from "./repo-list/LoadingRows";
import { RepoSettings } from "./repo-settings/RepoSettings";
import { RepoList } from "./repo-list/RepoList";
import { useDismissed } from "./repo-list/useDismissed";

/** How often "3 min ago" is recomputed while the popover is open. */
const CLOCK_TICK = 30_000;

/** What fills the window between header and footer. */
type View = { kind: "list" } | { kind: "add" } | { kind: "settings"; path: string };

const LIST: View = { kind: "list" };

/** The whole tray window: header, the repo list, the add form or a repo's settings, footer. */
export function Popover({ api }: { api: TetherApi }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [view, setView] = useState<View>(LIST);
  const [actionError, setActionError] = useState<string | null>(null);
  // Clicked but not yet picked up by the worker, which coalesces clicks for a moment.
  const [requested, setRequested] = useState<ReadonlySet<string>>(new Set());
  // An add, a save or a removal running in the form that fills the window.
  const [isFormPending, setFormPending] = useState(false);
  const now = useNow(CLOCK_TICK);
  // Every report reaches this hook, whichever view shows, so a dismissal clears as soon as
  // its repo stops needing you.
  const { isDismissed, dismiss } = useDismissed(snapshot?.report?.repos, snapshot?.details);

  const show = (next: View) => {
    setView(next);
    setFormPending(false);
  };

  const receive = useCallback((s: Snapshot) => {
    setSnapshot(s);
    setRequested(new Set());
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

  const run = useCallback((action: Promise<void>) => {
    setActionError(null);
    action.catch((e: unknown) => setActionError(String(e)));
  }, []);

  const sync = (path: string | null) => {
    const rows = snapshot?.report?.repos ?? [];
    setRequested(new Set(path === null ? rows.map((r) => r.path) : [path]));
    run(api.sync(path));
  };

  const report = snapshot?.report ?? null;
  const readError = snapshot && !report ? snapshot.error : null;
  const isEmptyList = report !== null && report.repos.length === 0;
  const retry = () => api.report().then(receive, (e: unknown) => setActionError(String(e)));
  const isBusy = (path: string) => requested.has(path) || (snapshot?.busy.includes(path) ?? false);
  // Gone when it was removed, here or by another writer; the list is shown instead.
  const settingsRow =
    view.kind === "settings" ? report?.repos.find((row) => row.path === view.path) : undefined;

  const body = (() => {
    if (view.kind === "add") {
      return (
        <AddForm
          api={api}
          onDone={() => show(LIST)}
          onCancel={() => show(LIST)}
          onPendingChange={setFormPending}
        />
      );
    }
    if (settingsRow) {
      return (
        <RepoSettings
          key={settingsRow.path}
          row={settingsRow}
          detail={snapshot?.details[settingsRow.path]}
          now={now}
          api={api}
          onDone={() => show(LIST)}
          onCancel={() => show(LIST)}
          onPendingChange={setFormPending}
        />
      );
    }
    if (snapshot === null) {
      return (
        <>
          <LoadingRows />
          <Footer
            hasRepos={false}
            isLoading
            paused={false}
            onSyncAll={() => sync(null)}
            onTogglePause={() => run(api.togglePause())}
            onAdd={() => show({ kind: "add" })}
          />
        </>
      );
    }
    if (readError) {
      return (
        <ReadError
          title="Cannot read repos.toml"
          message={readError}
          hint="Fix the file, then try again. Nothing syncs until it reads."
          onRetry={() => void retry()}
        />
      );
    }
    return (
      <>
        <RepoList
          rows={report?.repos ?? []}
          details={snapshot.details}
          isBusy={isBusy}
          isDismissed={isDismissed}
          paused={report?.paused ?? false}
          now={now}
          onSync={sync}
          onOpen={(path) => run(api.openInFinder(path))}
          onSettings={(path) => show({ kind: "settings", path })}
          onDismiss={dismiss}
          onAdd={() => show({ kind: "add" })}
        />
        <Footer
          hasRepos={(report?.repos.length ?? 0) > 0}
          isSyncing={snapshot.syncing}
          paused={report?.paused ?? false}
          onSyncAll={() => sync(null)}
          onTogglePause={() => run(api.togglePause())}
          onAdd={() => show({ kind: "add" })}
        />
      </>
    );
  })();

  const isListShown = view.kind === "list" || (view.kind === "settings" && !settingsRow);
  const isPending = isFormPending || (isListShown && (snapshot?.syncing ?? false));

  return (
    <PopoverFrame>
        <Header
          snapshot={snapshot}
          now={now}
          isPending={isPending}
          {...(view.kind === "add" ? { title: "Add repository", showState: false } : {})}
          {...(settingsRow ? { title: settingsRow.name, showState: false, repo: settingsRow } : {})}
          {...(isListShown && isEmptyList ? { showState: false } : {})}
        />
        <NoticeStrip
          text={noticeOf({
            actionError,
            note: snapshot?.note ?? null,
            staleError: readError ? null : (snapshot?.error ?? null),
          })}
        />
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
