import type { ComponentType, SVGProps } from "react";

import {
  ArrowDownToLineIcon,
  CheckCircleIcon,
  MinusCircleIcon,
  PauseCircleIcon,
  SpinnerIcon,
  WarningIcon,
} from "@steading/tray-popover/controls/icons";

import type { Health } from "./api";

/** The CLI's words for each state (tether-core `Health::label`). */
export const STATE_LABELS: Record<Health, string> = {
  "in-sync": "in sync",
  behind: "behind",
  skipped: "skipped",
  "needs-you": "needs you",
};

/** What leads a row or the header: a repo's state, a sync running, or tether paused. */
export type Glyph = Health | "syncing" | "paused";

export const GLYPHS: Record<Glyph, ComponentType<SVGProps<SVGSVGElement>>> = {
  "in-sync": CheckCircleIcon,
  behind: ArrowDownToLineIcon,
  skipped: MinusCircleIcon,
  "needs-you": WarningIcon,
  syncing: SpinnerIcon,
  paused: PauseCircleIcon,
};

/** Each glyph in its colour; the syncing arc also spins. */
export const GLYPH_CLASSES: Record<Glyph, string> = {
  "in-sync": "text-in-sync",
  behind: "text-behind",
  skipped: "text-skipped",
  "needs-you": "text-needs-you",
  syncing: "animate-spin text-behind",
  paused: "text-faint",
};

/** A running sync wins over paused, which wins over the repo's own state. */
export function glyphOf(health: Health, busy: boolean, paused: boolean): Glyph {
  if (busy) return "syncing";
  if (paused) return "paused";
  return health;
}

const HEADER_WORDS: Partial<Record<Glyph, string>> = { syncing: "syncing…", paused: "paused" };

/** Colour only where something differs: in sync stays quiet. */
const WORD_CLASSES: Record<Health, string> = {
  "in-sync": "text-faint",
  behind: "text-behind",
  skipped: "text-skipped",
  "needs-you": "text-needs-you",
};

interface StateLabelProps {
  health: Health;
  /** Commits to pull; adds ` · N` after `behind`. */
  behind?: number | undefined;
  /** A sync of this repo is running: `syncing…`. */
  busy?: boolean;
  /** tether is paused; only the header tone shows it. */
  paused?: boolean;
  /** Replaces the CLI's word, as the header's `needs you (2)`. */
  label?: string;
  /** `row`: the word alone (the row draws its glyph). `header`: glyph and word. */
  tone?: "row" | "header";
}

/** A state as the CLI's word, with its glyph in the header, so colour never carries it alone. */
export function StateLabel({
  health,
  behind,
  busy = false,
  paused = false,
  label,
  tone = "row",
}: StateLabelProps) {
  if (tone === "header") {
    const glyph = glyphOf(health, busy, paused);
    const Icon = GLYPHS[glyph];
    const word = HEADER_WORDS[glyph] ?? label ?? STATE_LABELS[health];
    const wordClass = headerWordClass(health, busy, paused);
    return (
      <span className="inline-flex min-w-0 items-center gap-2 whitespace-nowrap">
        <Icon className={`shrink-0 ${GLYPH_CLASSES[glyph]}`} />
        <span>
          <span className={wordClass}>{word}</span>
          {glyph === health && <BehindCount health={health} behind={behind} />}
        </span>
      </span>
    );
  }
  if (busy) return <span className="whitespace-nowrap text-behind">syncing…</span>;
  return (
    <span className="whitespace-nowrap">
      <span className={WORD_CLASSES[health]}>{label ?? STATE_LABELS[health]}</span>
      <BehindCount health={health} behind={behind} />
    </span>
  );
}

function BehindCount({ health, behind }: { health: Health; behind?: number | undefined }) {
  if (health !== "behind" || behind === undefined) return null;
  return <span className="text-faint"> · {behind}</span>;
}

function headerWordClass(health: Health, busy: boolean, paused: boolean): string {
  if (busy) return "text-behind";
  if (paused || health === "in-sync") return "text-muted";
  return WORD_CLASSES[health];
}
