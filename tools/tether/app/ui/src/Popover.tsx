import { useCallback, useEffect, useState } from "react";

import { NoticeStrip } from "@steading/tray-popover/NoticeStrip";
import { PopoverFooter } from "@steading/tray-popover/PopoverFooter";
import { PopoverFrame } from "@steading/tray-popover/PopoverFrame";
import { ReadError } from "@steading/tray-popover/ReadError";
import { WhereToLook, WhereToLookFooter } from "@steading/tray-popover/WhereToLook";

import { AddForm } from "./add-form/AddForm";
import type { Snapshot, TetherApi } from "./api";
import { Footer } from "./Footer";
import { Header } from "./Header";
import { noticeOf } from "./noticeOf";
import { placementOf } from "./repo-list/discoveryCopy";
import { EmptyState } from "./repo-list/EmptyState";
import { Found } from "./repo-list/Found";
import { LoadingRows } from "./repo-list/LoadingRows";
import { RepoSettings } from "./repo-settings/RepoSettings";
import { RepoList } from "./repo-list/RepoList";
import { useDismissed } from "./repo-list/useDismissed";

/** How often "3 min ago" is recomputed while the popover is open. */
const CLOCK_TICK = 30_000;

/** What fills the window between header and footer. */
type View = { kind: "list" } | { kind: "add" } | { kind: "settings"; path: string } | { kind: "roots" };

const LIST: View = { kind: "list" };

/**
 * Calls `onShown` each time the popover is shown again. `visibilitychange`, not window focus:
 * the folder picker also hands focus back, and that is not a new session.
 */
function useShownAgain(onShown: () => void) {
  useEffect(() => {
    const onChange = () => {
      if (document.visibilityState === "visible") onShown();
    };
    document.addEventListener("visibilitychange", onChange);
    return () => document.removeEventListener("visibilitychange", onChange);
  }, [onShown]);
}

const cloneCount = (n: number) => `${n} ${n === 1 ? "clone" : "clones"}`;

/** The whole tray window: header, the repo list, the add form or a repo's settings, footer. */
export function Popover({ api }: { api: TetherApi }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [view, setView] = useState<View>(LIST);
  const [actionError, setActionError] = useState<string | null>(null);
  // Clicked but not yet picked up by the worker, which coalesces clicks for a moment.
  const [requested, setRequested] = useState<ReadonlySet<string>>(new Set());
  // An add, a save or a removal running in the form that fills the window.
  const [isFormPending, setFormPending] = useState(false);
  // Suggestions whose *Add* is in flight: the row stays, with *Add* disabled.
  const [adding, setAdding] = useState<ReadonlySet<string>>(new Set());
  // This popover session has answered a suggestion or started a scan: Found stays open.
  const [hasActed, setActed] = useState(false);
  const now = useNow(CLOCK_TICK);
  // A new session starts when the popover is shown again; Found then folds up to one row.
  useShownAgain(useCallback(() => setActed(false), []));
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

  /** Runs an action; a failure shows in the notice strip. Resolves once it has settled either way. */
  const run = useCallback((action: Promise<void>) => {
    setActionError(null);
    return action.then(
      () => {},
      (e: unknown) => setActionError(String(e)),
    );
  }, []);

  const findClones = () => {
    setActed(true);
    void run(api.findClones());
  };
  const addSuggestion = (path: string) => {
    setActed(true);
    setAdding((current) => new Set(current).add(path));
    void run(api.addSuggestion(path)).then(() =>
      setAdding((current) => new Set([...current].filter((p) => p !== path))),
    );
  };
  const skipClone = (path: string) => {
    setActed(true);
    void run(api.skipClone(path));
  };
  const addScanRoot = () =>
    void run(api.pickFolder().then((picked) => (picked === null ? undefined : api.addScanRoot(picked))));

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
    const discovery = snapshot.discovery;
    if (view.kind === "roots") {
      return (
        <>
          <div className="min-h-0 flex-1 overflow-y-auto">
            <WhereToLook
              foldersTitle="Folders"
              roots={discovery.roots.map((root) => ({
                path: root.path,
                found: root.clones === null ? undefined : cloneCount(root.clones),
                error: root.error ?? undefined,
              }))}
              addFolderLabel="Add folder…"
              onAddRoot={addScanRoot}
              onRemoveRoot={(path) => void run(api.removeScanRoot(path))}
              removeRootLabel={(path) => `Stop looking in ${path}`}
              note="Desktop, Documents and Downloads are searched only if you add them here. macOS asks you first."
              skippedTitle="Skipped"
              skipped={discovery.skipped}
              unskipLabel="Unskip"
              onUnskip={(path) => void run(api.unskipClone(path))}
            />
          </div>
          <PopoverFooter>
            <WhereToLookFooter
              doneLabel="Done"
              onDone={() => show(LIST)}
              lookAgainLabel="Look again"
              onLookAgain={() => {
                findClones();
                show(LIST);
              }}
              lookAgainDisabled={discovery.roots.length === 0}
            />
          </PopoverFooter>
        </>
      );
    }
    const hasRepos = (report?.repos.length ?? 0) > 0;
    const placement = placementOf(discovery, hasRepos, hasActed);
    const whereToLook = () => show({ kind: "roots" });
    const found =
      placement === "open" || placement === "collapsed" || placement === "settled" ? (
        <Found
          discovery={discovery}
          mode={placement}
          hasRepos={hasRepos}
          now={now}
          adding={adding}
          onAdd={addSuggestion}
          onSkip={skipClone}
          onLookAgain={findClones}
          onWhereToLook={whereToLook}
        />
      ) : undefined;
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
          empty={
            <EmptyState
              variant={placement === "found-nothing" ? "found-nothing" : "invite"}
              discovery={discovery}
              now={now}
              onAdd={() => show({ kind: "add" })}
              onFind={findClones}
              onWhereToLook={whereToLook}
            />
          }
          found={found}
        />
        <Footer
          hasRepos={hasRepos}
          isSyncing={snapshot.syncing}
          isFoundBody={!hasRepos && found !== undefined}
          paused={report?.paused ?? false}
          onSyncAll={() => sync(null)}
          onTogglePause={() => run(api.togglePause())}
          onAdd={() => show({ kind: "add" })}
        />
      </>
    );
  })();

  const isListShown = view.kind === "list" || (view.kind === "settings" && !settingsRow);
  const isScanning = snapshot?.discovery.scanning != null;
  const isPending =
    isFormPending || adding.size > 0 || (isListShown && ((snapshot?.syncing ?? false) || isScanning));

  return (
    <PopoverFrame>
        <Header
          snapshot={snapshot}
          now={now}
          isPending={isPending}
          {...(view.kind === "add" ? { title: "Add repository", showState: false } : {})}
          {...(view.kind === "roots" ? { title: "Where to look", showState: false } : {})}
          {...(settingsRow ? { title: settingsRow.name, showState: false, repo: settingsRow } : {})}
          {...(isListShown && isEmptyList ? { showState: false, showFound: true } : {})}
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
