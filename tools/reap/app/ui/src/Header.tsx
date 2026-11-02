import type { Report, Snapshot } from "./api";
import { shownSize, totalSizeOf } from "./shownSize";
import { OVERALL_LABELS, OVERALL_TONES, StateLabel } from "./StateLabel";

interface HeaderProps {
  snapshot: Snapshot | null;
  title?: string;
  /** False in the add view, which shows the title alone. */
  showState?: boolean;
}

/** The title, the overall state as a dot and a word, and a quiet summary with the disk taken on the right. */
export function Header({ snapshot, title = "reap", showState = true }: HeaderProps) {
  const rightSlot = showState ? summaryOf(snapshot) : null;
  return (
    <header className="flex items-center gap-2 border-b border-border px-4 py-3">
      <h1 className="text-base font-semibold text-text">{title}</h1>
      {showState && (
        <span className="text-sm text-muted" role="status">
          <OverallState snapshot={snapshot} />
        </span>
      )}
      {rightSlot && <span className="ml-auto text-xs text-faint">{rightSlot}</span>}
    </header>
  );
}

function summaryOf(snapshot: Snapshot | null): string | null {
  const report: Report | null = snapshot?.report ?? null;
  if (!report) return null;
  if (report.paused) return "Paused";
  if (report.repos.length === 0) return null;
  const worktrees = report.repos.flatMap((repo) => repo.worktrees);
  const counts = `${plural(worktrees.length, "worktree")} · ${plural(report.repos.length, "repo")}`;
  const size = totalSizeOf(worktrees);
  return size > 0 ? `${counts} · ${shownSize(size)}` : counts;
}

function plural(count: number, noun: string): string {
  return `${count} ${count === 1 ? noun : `${noun}s`}`;
}

function OverallState({ snapshot }: { snapshot: Snapshot | null }) {
  if (!snapshot) return <>checking…</>;
  if (snapshot.error) return <StateLabel tone="needs-you" label={OVERALL_LABELS["needs-you"]} />;
  const report = snapshot.report;
  if (!report) return <>checking…</>;
  if (report.overall === "needs-you") {
    return <StateLabel tone="needs-you" label={`${OVERALL_LABELS["needs-you"]} (${report.needs_you})`} />;
  }
  if (snapshot.refreshing) return <>refreshing…</>;
  return <StateLabel tone={OVERALL_TONES[report.overall]} label={OVERALL_LABELS[report.overall]} />;
}
