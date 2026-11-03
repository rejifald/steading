import type { ComponentType, SVGProps } from "react";

import {
  BinIcon,
  CheckCircleIcon,
  DashedCircleIcon,
  PauseCircleIcon,
  PinIcon,
  SpinnerIcon,
  WarningIcon,
} from "@steading/tray-popover/controls/icons";

import type { Overall, State } from "./api";
import type { Outcome } from "./worktree-list/outcomeOf";

/** The CLI's words for each worktree state. */
export const STATE_LABELS: Record<State, string> = {
  open: "open",
  removable: "removable",
  dirty: "dirty",
  "in-use": "in use",
  locked: "locked",
  stale: "stale",
  missing: "missing",
  kept: "kept",
  refused: "refused",
};

export const OVERALL_LABELS: Record<Overall, string> = {
  tidy: "tidy",
  pending: "removal pending",
  "needs-you": "needs you",
};

/** What leads a section heading or the header: an outcome, an overall state, or what reap is doing. */
export type Glyph = Outcome | "tidy" | "pending" | "refreshing" | "paused";

export const GLYPHS: Record<Glyph, ComponentType<SVGProps<SVGSVGElement>>> = {
  "needs-you": WarningIcon,
  ready: BinIcon,
  held: PauseCircleIcon,
  "in-progress": DashedCircleIcon,
  kept: PinIcon,
  tidy: CheckCircleIcon,
  pending: BinIcon,
  refreshing: SpinnerIcon,
  paused: PauseCircleIcon,
};

/** Each glyph in its colour; the refreshing arc also spins. */
export const GLYPH_CLASSES: Record<Glyph, string> = {
  "needs-you": "text-needs-you",
  ready: "text-removable",
  held: "text-held",
  "in-progress": "text-open",
  kept: "text-open",
  tidy: "text-removable",
  pending: "text-held",
  refreshing: "animate-spin text-progress",
  paused: "text-faint",
};

export const OVERALL_GLYPHS: Record<Overall, Glyph> = {
  tidy: "tidy",
  pending: "pending",
  "needs-you": "needs-you",
};

/** The word's colour: quiet where nothing needs looking at, else the glyph's. */
const WORD_CLASSES: Record<Glyph, string> = {
  ...GLYPH_CLASSES,
  tidy: "text-muted",
  paused: "text-muted",
  refreshing: "text-progress",
};

/** A glyph alone, in its colour, hidden from assistive technology (the words beside it say it). */
export function GlyphIcon({ glyph }: { glyph: Glyph }) {
  const Icon = GLYPHS[glyph];
  return <Icon className={`shrink-0 ${GLYPH_CLASSES[glyph]}`} />;
}

/** A state as a glyph and its word, so colour never carries it alone. */
export function StateLabel({ glyph, label }: { glyph: Glyph; label: string }) {
  return (
    <span className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap">
      <GlyphIcon glyph={glyph} />
      <span className={WORD_CLASSES[glyph]}>{label}</span>
    </span>
  );
}
