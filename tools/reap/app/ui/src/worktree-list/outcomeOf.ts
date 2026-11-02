import type { State } from "../api";
import type { Tone } from "../StateLabel";

/** What the popover groups a worktree under: the question it answers for the owner. */
export type Outcome = "needs-you" | "ready" | "held" | "in-progress" | "kept";

/** The sections' order, top to bottom. */
export const OUTCOMES: readonly Outcome[] = ["needs-you", "ready", "held", "in-progress", "kept"];

const OUTCOME_OF_STATE: Record<State, Outcome> = {
  stale: "needs-you",
  refused: "needs-you",
  removable: "ready",
  missing: "ready",
  dirty: "held",
  "in-use": "held",
  locked: "held",
  open: "in-progress",
  kept: "kept",
};

/** The colour of a section's dot. */
export const OUTCOME_TONES: Record<Outcome, Tone> = {
  "needs-you": "needs-you",
  ready: "removable",
  held: "held",
  "in-progress": "open",
  kept: "open",
};

/** The section a state belongs to. */
export function outcomeOf(state: State): Outcome {
  return OUTCOME_OF_STATE[state];
}
