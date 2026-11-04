import { PopoverHeader } from "@steading/tray-popover/PopoverHeader";

import type { Row, Snapshot } from "./api";
import { ago } from "./repo-list/ago";
import { StateLabel } from "./StateLabel";

interface HeaderProps {
  snapshot: Snapshot | null;
  now: Date;
  title?: string;
  /** False in the add view and the empty list, which show the title alone. */
  showState?: boolean;
  /** In a repo's settings: that repo's state on the right, as its row shows it. */
  repo?: Row;
  /** Draws the progress hairline on the bottom edge: a sync, an add, a save or a removal. */
  isPending?: boolean;
}

/** The title, the overall state as a glyph and a word, and a quiet summary on the right. */
export function Header({ snapshot, now, title = "tether", showState = true, repo, isPending = false }: HeaderProps) {
  const rightSlot = showState ? summaryOf(snapshot, now) : null;
  const aside = repo ? (
    <span className="text-sm">
      <StateLabel tone="header" health={repo.health} behind={repo.behind} />
    </span>
  ) : (
    rightSlot && <span className="text-xs text-faint">{rightSlot}</span>
  );
  return (
    <PopoverHeader
      title={title}
      state={showState ? <OverallState snapshot={snapshot} /> : undefined}
      aside={aside}
      isPending={isPending}
    />
  );
}

function summaryOf(snapshot: Snapshot | null, now: Date): string | null {
  const report = snapshot?.report ?? null;
  if (!snapshot || !report) return null;
  const { repos } = report;
  if (snapshot.syncing) return `${snapshot.busy.length} of ${repos.length}`;
  if (repos.length === 0) return null;
  if (report.paused || repos.some((row) => row.health !== "in-sync")) {
    return `${repos.length} ${repos.length === 1 ? "repo" : "repos"}`;
  }
  const newest = repos
    .map((row) => row.last_sync)
    .filter((time): time is string => time !== undefined)
    .sort()
    .at(-1);
  return `last sync ${ago(newest, now)}`;
}

/** First match wins: read error, paused, needs you, syncing, the overall health. */
function OverallState({ snapshot }: { snapshot: Snapshot | null }) {
  const report = snapshot?.report ?? null;
  if (snapshot?.error && !report) return <StateLabel tone="header" health="needs-you" />;
  if (!report) return <span className="text-muted">checking…</span>;
  if (report.paused) return <StateLabel tone="header" health={report.overall} paused />;
  if (report.needs_you > 0) {
    return <StateLabel tone="header" health="needs-you" label={`needs you (${report.needs_you})`} />;
  }
  if (snapshot?.syncing) return <StateLabel tone="header" health={report.overall} busy />;
  return <StateLabel tone="header" health={report.overall} />;
}
