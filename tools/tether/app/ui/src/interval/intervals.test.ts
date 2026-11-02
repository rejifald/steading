import { describe, expect, it } from "vitest";

import { choiceOf, choicesFor, PRESETS } from "./intervals";

describe("choicesFor", () => {
  it("is the presets alone for a preset interval", () => {
    expect(choicesFor(300)).toEqual(PRESETS);
  });

  it("adds a custom interval from repos.toml in order, spelled as tether-core reads it", () => {
    expect(choicesFor(90).map((choice) => [choice.value, choice.label])).toEqual([
      ["1m", "1 min"],
      ["90s", "90 s (custom)"],
      ["5m", "5 min"],
      ["15m", "15 min"],
      ["1h", "1 hour"],
    ]);
  });

  it("spells whole minutes and hours with their unit", () => {
    expect(choiceOf(600).value).toBe("10m");
    expect(choiceOf(7200).value).toBe("2h");
  });
});
