import type { Discovery } from "../api";

export type FoundMode = "open" | "collapsed" | "settled";

/**
 * How the Found section sits in the popover, or `null` for none (anatomy §6).
 *
 * `isEngaged` is true once this popover session has started a scan or answered a suggestion, so
 * the section the operator is working in does not fold away under them.
 */
export function foundModeOf({
  discovery,
  hasRepos,
  isEngaged,
}: {
  discovery: Discovery;
  hasRepos: boolean;
  isEngaged: boolean;
}): FoundMode | null {
  const isScanning = discovery.scanning !== null;
  const hasSuggestions = discovery.suggestions.length > 0;
  // With nothing listed, Found is the whole body while there is anything to show; the empty
  // state (invitation or found-nothing) takes over otherwise.
  if (!hasRepos) return isScanning || hasSuggestions ? "open" : null;
  if (isScanning) return "open";
  if (hasSuggestions) return isEngaged ? "open" : "collapsed";
  return discovery.scanned_at !== null ? "settled" : null;
}
