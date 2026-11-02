import { ScrollArea } from "@base-ui/react/scroll-area";
import { useState } from "react";

import type { Repo, State, Worktree, WorktreeAction } from "../api";
import { shownSize, totalSizeOf } from "../shownSize";
import { EmptyState } from "./EmptyState";
import { NothingToRemove } from "./NothingToRemove";
import { type Outcome, outcomeOf } from "./outcomeOf";
import { OutcomeSection } from "./OutcomeSection";
import { RemoveAllButton, RemoveAllConfirm } from "./RemoveAll";
import { RepoErrorRow } from "./RepoErrorRow";
import { RepoNotes } from "./RepoNotes";
import { WorktreeRow } from "./WorktreeRow";

/** Ready rows shown before "Show N more". */
const READY_ROW_LIMIT = 4;

interface WorktreeListProps {
  repos: Repo[];
  now: Date;
  onAction: (action: WorktreeAction, path: string) => Promise<boolean> | void;
  onAdd: () => void;
}

interface Entry {
  repo: Repo;
  worktree: Worktree;
}

/** Every worktree of one outcome: repo order as reported, then worktree order. */
function entriesOf(repos: Repo[], outcome: Outcome): Entry[] {
  return repos.flatMap((repo) =>
    repo.worktrees.filter((worktree) => outcomeOf(worktree.state) === outcome).map((worktree) => ({ repo, worktree })),
  );
}

const HELD_KINDS: { state: State; label: string }[] = [
  { state: "in-use", label: "in use" },
  { state: "dirty", label: "uncommitted" },
  { state: "locked", label: "locked" },
];

/** The disk a set of entries takes, as shown; `undefined` when none was measured. */
function sizeOf(entries: Entry[]): string | undefined {
  const size = totalSizeOf(entries.map(({ worktree }) => worktree));
  return size > 0 ? shownSize(size) : undefined;
}

/** "8 in use · 4 uncommitted (9.3 GB) · 1 locked": only the kinds that are present, and the
 * disk held by uncommitted changes, which only the owner can free. */
function heldSummaryOf(entries: Entry[]): string {
  return HELD_KINDS.map(({ state, label }) => {
    const ofKind = entries.filter(({ worktree }) => worktree.state === state);
    const size = state === "dirty" ? sizeOf(ofKind) : undefined;
    return { count: ofKind.length, text: size ? `${ofKind.length} ${label} (${size})` : `${ofKind.length} ${label}` };
  })
    .filter(({ count }) => count > 0)
    .map(({ text }) => text)
    .join(" · ");
}

const READY_LEADS: Record<Repo["policy"], string> = {
  ask: "Merged and clean. You remove them here; reap waits (policy: ask).",
  auto: "Merged and clean. reap removes each one when its grace period ends.",
  off: "Merged and clean. Auto-removal is off; remove them here.",
};

/** The lead sentence of Ready to remove: by policy when every repo agrees, else a bare one. */
function readyLeadOf(entries: Entry[]): string {
  const policies = new Set(entries.map(({ repo }) => repo.policy));
  const [only] = policies;
  if (policies.size !== 1 || only === undefined) return "Merged and clean.";
  return READY_LEADS[only];
}

/** The scrolling middle of the popover: worktrees grouped by outcome across repos. */
export function WorktreeList({ repos, now, onAction, onAdd }: WorktreeListProps) {
  const [isConfirmingRemoveAll, setIsConfirmingRemoveAll] = useState(false);
  if (repos.length === 0) return <EmptyState onAdd={onAdd} />;
  const needsYou = entriesOf(repos, "needs-you");
  const reposWithError = repos.filter((repo) => repo.error);
  const needsYouCount = needsYou.length + reposWithError.length;
  const ready = entriesOf(repos, "ready");
  // Only removable worktrees can be removed by hand; a missing one is cleared by a sweep.
  const removablePaths = ready.filter(({ worktree }) => worktree.state === "removable").map(({ worktree }) => worktree.path);
  const held = entriesOf(repos, "held");
  const inProgress = entriesOf(repos, "in-progress");
  const kept = entriesOf(repos, "kept");
  const row = ({ repo, worktree }: Entry, emphasis = false) => (
    <WorktreeRow
      key={`${repo.path}:${worktree.path}`}
      worktree={worktree}
      repo={repo}
      now={now}
      onAction={onAction}
      emphasis={emphasis}
    />
  );

  return (
    <ScrollArea.Root className="relative min-h-0 flex-1">
      <ScrollArea.Viewport className="h-full focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus">
        <OutcomeSection outcome="needs-you" title="Needs you" count={needsYouCount}>
          {reposWithError.map((repo) => (
            <RepoErrorRow key={repo.path} repo={repo} onAction={onAction} />
          ))}
          {needsYou.map((entry) => row(entry, true))}
        </OutcomeSection>
        {needsYouCount === 0 && ready.length === 0 && <NothingToRemove />}
        <OutcomeSection
          outcome="ready"
          title="Ready to remove"
          count={ready.length}
          size={sizeOf(ready)}
          lead={
            isConfirmingRemoveAll ? (
              <RemoveAllConfirm
                paths={removablePaths}
                onAction={onAction}
                onClose={() => setIsConfirmingRemoveAll(false)}
              />
            ) : (
              readyLeadOf(ready)
            )
          }
          action={
            isConfirmingRemoveAll || removablePaths.length === 0 ? null : (
              <RemoveAllButton count={removablePaths.length} onClick={() => setIsConfirmingRemoveAll(true)} />
            )
          }
          limit={READY_ROW_LIMIT}
        >
          {ready.map((entry) => row(entry))}
        </OutcomeSection>
        <OutcomeSection outcome="held" title="Merged, but held" count={held.length} collapsible summary={heldSummaryOf(held)}>
          {held.map((entry) => row(entry))}
        </OutcomeSection>
        <OutcomeSection outcome="in-progress" title="In progress" count={inProgress.length} collapsible summary="not merged yet">
          {inProgress.map((entry) => row(entry))}
        </OutcomeSection>
        <OutcomeSection outcome="kept" title="Kept" count={kept.length} collapsible summary="until new commits">
          {kept.map((entry) => row(entry))}
        </OutcomeSection>
        <RepoNotes repos={repos} />
      </ScrollArea.Viewport>
      <ScrollArea.Scrollbar className="flex w-2 justify-center py-1 opacity-0 data-hovering:opacity-100 data-scrolling:opacity-100">
        <ScrollArea.Thumb className="w-full rounded-full bg-border" />
      </ScrollArea.Scrollbar>
    </ScrollArea.Root>
  );
}
