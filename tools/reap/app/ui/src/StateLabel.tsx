import type { Overall, State } from "./api";

/** The colour a state is drawn in; several states share one. */
export type Tone = "open" | "removable" | "held" | "needs-you";

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

export const STATE_TONES: Record<State, Tone> = {
  open: "open",
  removable: "removable",
  missing: "removable",
  dirty: "held",
  "in-use": "held",
  locked: "held",
  stale: "needs-you",
  kept: "open",
  refused: "needs-you",
};

export const OVERALL_LABELS: Record<Overall, string> = {
  tidy: "tidy",
  pending: "removal pending",
  "needs-you": "needs you",
};

export const OVERALL_TONES: Record<Overall, Tone> = {
  tidy: "removable",
  pending: "held",
  "needs-you": "needs-you",
};

/** The background class of each tone's dot. */
export const DOT_COLOURS: Record<Tone, string> = {
  open: "bg-open",
  removable: "bg-removable",
  held: "bg-held",
  "needs-you": "bg-needs-you",
};

/** A state as a dot and its word, so colour never carries it alone. */
export function StateLabel({ tone, label }: { tone: Tone; label: string }) {
  return (
    <span className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap">
      <span aria-hidden className={`size-2 shrink-0 rounded-full ${DOT_COLOURS[tone]}`} />
      <span>{label}</span>
    </span>
  );
}
