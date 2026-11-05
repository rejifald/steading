import { FoundRow, FoundSection } from "@steading/tray-popover/FoundSection";
import { TextButton } from "@steading/tray-popover/controls/TextButton";

import type { Discovery } from "../api";
import { lookedAgo } from "../worktree-list/ago";
import { detailOf, errorLinesOf, leadOf, settledTextOf } from "./copy";
import type { FoundMode } from "./foundMode";

/** Placeholder rows after the real ones while a scan runs. */
const GHOST_ROWS = 3;

interface FoundAreaProps {
  discovery: Discovery;
  mode: FoundMode;
  now: Date;
  /** Listed rows sit above: draw the rule that separates them. */
  hasRuleAbove: boolean;
  /** Clones whose *Add* is in flight. */
  adding: ReadonlySet<string>;
  onAdd: (path: string) => void;
  onSkip: (path: string) => void;
  onLookAgain: () => void;
  onWhereToLook: () => void;
}

/**
 * reap's Found section: the clones with linked worktrees a scan found, with *Add* and ✕ on each.
 * Found is not an outcome section; it holds clones, not a worktree's fate.
 */
export function FoundArea({
  discovery,
  mode,
  now,
  hasRuleAbove,
  adding,
  onAdd,
  onSkip,
  onLookAgain,
  onWhereToLook,
}: FoundAreaProps) {
  const { suggestions, scanning, scanned_at: scannedAt } = discovery;
  return (
    <FoundSection
      mode={mode}
      title="Found"
      count={suggestions.length}
      lead={leadOf(discovery, now)}
      aside={
        <TextButton size="compact" variant="quiet" onClick={onWhereToLook}>
          Where to look…
        </TextButton>
      }
      scanning={scanning === null ? undefined : `looking in ${scanning}…`}
      errors={errorLinesOf(discovery.roots)}
      ghostRows={scanning === null ? 0 : GHOST_ROWS}
      collapsedSummary={scannedAt === null ? undefined : `· looked ${lookedAgo(scannedAt, now)}`}
      settledText={settledTextOf(discovery, now)}
      onLookAgain={onLookAgain}
      lookAgainLabel="Look again"
      hasRuleAbove={hasRuleAbove}
    >
      {suggestions.map((suggestion) => (
        <FoundRow
          key={suggestion.path}
          name={suggestion.name}
          detail={detailOf(suggestion)}
          title={suggestion.path}
          addLabel="Add"
          onAdd={() => onAdd(suggestion.path)}
          skipLabel={`Skip ${suggestion.name}`}
          onSkip={() => onSkip(suggestion.path)}
          busy={adding.has(suggestion.path)}
        />
      ))}
    </FoundSection>
  );
}
