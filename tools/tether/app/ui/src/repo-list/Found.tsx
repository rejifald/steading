import { TextButton } from "@steading/tray-popover/controls/TextButton";
import { FoundRow, FoundSection } from "@steading/tray-popover/FoundSection";

import type { Discovery } from "../api";
import { collapsedSummaryOf, errorLinesOf, leadOf, settledTextOf } from "./discoveryCopy";

/** Placeholder rows under the real ones while a scan runs. */
const GHOST_ROWS = 3;

interface FoundProps {
  discovery: Discovery;
  mode: "open" | "collapsed" | "settled";
  /** Listed rows sit above: the rule that separates them. */
  hasRepos: boolean;
  now: Date;
  /** Suggestions whose *Add* is in flight. */
  adding: ReadonlySet<string>;
  onAdd: (path: string) => void;
  onSkip: (path: string) => void;
  onLookAgain: () => void;
  onWhereToLook: () => void;
}

/** tether's words around the shared `FoundSection`: each suggestion with the branch *Add* would follow. */
export function Found({
  discovery,
  mode,
  hasRepos,
  now,
  adding,
  onAdd,
  onSkip,
  onLookAgain,
  onWhereToLook,
}: FoundProps) {
  const isScanning = discovery.scanning !== null;
  return (
    <FoundSection
      mode={mode}
      title="Found"
      count={discovery.suggestions.length}
      lead={leadOf(discovery, now)}
      aside={
        <TextButton size="compact" variant="quiet" onClick={onWhereToLook}>
          Where to look…
        </TextButton>
      }
      scanning={isScanning ? `looking in ${discovery.scanning}…` : undefined}
      errors={errorLinesOf(discovery)}
      ghostRows={isScanning ? GHOST_ROWS : 0}
      collapsedSummary={collapsedSummaryOf(discovery, now)}
      settledText={settledTextOf(discovery, now)}
      onLookAgain={onLookAgain}
      lookAgainLabel="Look again"
      hasRuleAbove={hasRepos}
    >
      {discovery.suggestions.map((suggestion) => (
        <FoundRow
          key={suggestion.path}
          name={suggestion.name}
          detail={`${suggestion.branch} · ${suggestion.root}`}
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
