import { PopoverHeader } from "@steading/tray-popover/PopoverHeader";

import type { Report, Snapshot } from "./api";
import { shownSize, totalSizeOf } from "./shownSize";
import { OVERALL_GLYPHS, OVERALL_LABELS, StateLabel } from "./StateLabel";

interface HeaderProps {
  snapshot: Snapshot | null;
  title?: string;
  /** False in the add view, which shows the title alone. */
  showState?: boolean;
  /** Draws the progress hairline on the bottom edge: a refresh, a removal or an add. */
  isPending?: boolean;
}

/** The title, the overall state as a glyph and a word, and a quiet summary with the disk taken on the right. */
export function Header({ snapshot, title = "reap", showState = true, isPending = false }: HeaderProps) {
  const rightSlot = showState ? summaryOf(snapshot) : null;
  return (
    <PopoverHeader
      title={title}
      state={showState ? <OverallState snapshot={snapshot} /> : undefined}
      aside={rightSlot && <span className="text-xs text-faint">{rightSlot}</span>}
      isPending={isPending}
    />
  );
}

function summaryOf(snapshot: Snapshot | null): string | null {
  const report: Report | null = snapshot?.report ?? null;
  if (!report || report.repos.length === 0) return null;
  const worktrees = report.repos.flatMap((repo) => repo.worktrees);
  const counts = `${plural(worktrees.length, "worktree")} · ${plural(report.repos.length, "repo")}`;
  const size = totalSizeOf(worktrees);
  return size > 0 ? `${counts} · ${shownSize(size)}` : counts;
}

function plural(count: number, noun: string): string {
  return `${count} ${count === 1 ? noun : `${noun}s`}`;
}

/** First match wins: read error, paused, needs you, refreshing, the overall state. */
function OverallState({ snapshot }: { snapshot: Snapshot | null }) {
  const report = snapshot?.report ?? null;
  if (snapshot?.error && !report) return <StateLabel glyph="needs-you" label={OVERALL_LABELS["needs-you"]} />;
  if (!report) return <span className="text-muted">checking…</span>;
  if (report.paused) return <StateLabel glyph="paused" label="paused" />;
  if (report.overall === "needs-you") {
    return <StateLabel glyph="needs-you" label={`${OVERALL_LABELS["needs-you"]} (${report.needs_you})`} />;
  }
  if (snapshot?.refreshing) return <StateLabel glyph="refreshing" label="refreshing…" />;
  return <StateLabel glyph={OVERALL_GLYPHS[report.overall]} label={OVERALL_LABELS[report.overall]} />;
}
