import { type ReactNode, useState } from "react";

import type { Repo, State, Worktree, WorktreeAction } from "../api";
import { shownSize, totalSizeOf } from "../shownSize";
import { ListScroll } from "./ListScroll";
import { NothingToRemove } from "./NothingToRemove";
import { type Outcome, outcomeOf } from "./outcomeOf";
import { OutcomeSection } from "./OutcomeSection";
import { RemoveAllButton, RemoveAllConfirm } from "./RemoveAll";
import { RepoErrorRow } from "./RepoErrorRow";
import { RepoNotes } from "./RepoNotes";
import { SalvagedSection } from "./SalvagedSection";
import { WorktreeRow } from "./WorktreeRow";

/** Ready rows shown before "Show N more". */
const READY_ROW_LIMIT = 4;

const NONE_REMOVING: ReadonlySet<string> = new Set();

interface WorktreeListProps {
  repos: Repo[];
  now: Date;
  onAction: (action: WorktreeAction, path: string) => Promise<boolean> | void;
  /** Worktrees whose removal is running, by path. */
  removing?: ReadonlySet<string>;
  /** The Found section: the last thing in the scroll, after the outcome sections and the notes. */
  found?: ReactNode;
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
export function WorktreeList({ repos, now, onAction, removing = NONE_REMOVING, found = null }: WorktreeListProps) {
  const [isConfirmingRemoveAll, setIsConfirmingRemoveAll] = useState(false);
  const needsYou = entriesOf(repos, "needs-you");
  const reposWithError = repos.filter((repo) => repo.error);
  const needsYouCount = needsYou.length + reposWithError.length;
  const ready = entriesOf(repos, "ready");
  // Only removable worktrees can be removed by hand; a missing one is cleared by a sweep, and
  // one already being removed is not offered again.
  const removablePaths = ready
    .filter(({ worktree }) => worktree.state === "removable" && !removing.has(worktree.path))
    .map(({ worktree }) => worktree.path);
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
      isRemoving={removing.has(worktree.path)}
    />
  );

  return (
    <ListScroll>
      <OutcomeSection outcome="needs-you" title="Needs you" count={needsYouCount} isRaised>
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
      <SalvagedSection repos={repos} now={now} />
      <RepoNotes repos={repos} />
      {found}
    </ListScroll>
  );
}
