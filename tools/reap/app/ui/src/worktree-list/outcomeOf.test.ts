import { describe, expect, it } from "vitest";

import type { State } from "../api";
import { OUTCOMES, type Outcome, outcomeOf } from "./outcomeOf";

describe("outcomeOf", () => {
  it.each<[State, Outcome]>([
    ["stale", "needs-you"],
    ["refused", "needs-you"],
    ["removable", "ready"],
    ["missing", "ready"],
    ["dirty", "held"],
    ["in-use", "held"],
    ["locked", "held"],
    ["open", "in-progress"],
    ["kept", "kept"],
  ])("puts %s under %s", (state, outcome) => {
    expect(outcomeOf(state)).toBe(outcome);
  });

  it("orders the sections needs-you, ready, held, in-progress, kept", () => {
    expect(OUTCOMES).toEqual(["needs-you", "ready", "held", "in-progress", "kept"]);
  });
});
