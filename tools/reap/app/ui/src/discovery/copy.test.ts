import { describe, expect, it } from "vitest";

import type { Discovery, RootInfo } from "../api";
import { errorLinesOf, leadOf, lookedLineOf, rootsText, settledTextOf } from "./copy";

const NOW = new Date("2026-01-01T12:00:00Z");
const root = (path: string, fields: Partial<RootInfo> = {}): RootInfo => ({
  path,
  clones: 0,
  stopped_early: false,
  error: null,
  ...fields,
});
const discovery = (roots: RootInfo[], fields: Partial<Discovery> = {}): Discovery => ({
  scanned_at: NOW.toISOString(),
  scanning: null,
  roots,
  suggestions: [],
  skipped: [],
  ...fields,
});

describe("rootsText", () => {
  it("names one, two, and three or more roots", () => {
    expect(rootsText(["~/Development"])).toBe("~/Development");
    expect(rootsText(["~/Development", "~/Code"])).toBe("~/Development and ~/Code");
    expect(rootsText(["~/Development", "~/Code", "~/src", "~/work"])).toBe("~/Development, ~/Code and 2 more");
    expect(rootsText(["~/Development", "~/Code", "~/src"])).toBe("~/Development, ~/Code and 1 more");
  });

  it("is empty for none", () => {
    expect(rootsText([])).toBe("");
  });
});

describe("leadOf", () => {
  const roots = [root("~/Development"), root("~/Code")];

  it("tells the operator to go ahead while a scan runs", () => {
    expect(leadOf(discovery(roots, { scanning: "~/Code" }), NOW)).toBe(
      "You can add these now; more appear as the search goes.",
    );
  });

  it("names the roots searched and when, once done", () => {
    expect(leadOf(discovery(roots), NOW)).toBe(
      "Clones with worktrees in ~/Development and ~/Code, just now. Merged worktrees go 24 h after reap first sees them.",
    );
  });

  it("leaves out a root that could not be read", () => {
    expect(leadOf(discovery([root("~/Development"), root("~/work", { error: "permission denied" })]), NOW)).toContain(
      "in ~/Development, just now.",
    );
  });
});

describe("errorLinesOf", () => {
  it("names the unreadable root and the ones searched", () => {
    expect(
      errorLinesOf([root("~/Development"), root("~/Code"), root("~/work", { error: "permission denied" })]),
    ).toEqual(["Cannot read ~/work: permission denied. ~/Development and ~/Code were searched."]);
  });

  it("uses was for one searched root and says nothing of others when none was", () => {
    expect(errorLinesOf([root("~/Code"), root("~/work", { error: "gone" })])).toEqual([
      "Cannot read ~/work: gone. ~/Code was searched.",
    ]);
    expect(errorLinesOf([root("~/work", { error: "gone" })])).toEqual(["Cannot read ~/work: gone."]);
  });
});

describe("settledTextOf", () => {
  it("says no suggestions when the roots held no clone", () => {
    expect(settledTextOf(discovery([root("~/Development")]), NOW)).toBe("No suggestions · looked just now");
  });

  it("says nothing new when the roots held clones that are all answered", () => {
    expect(settledTextOf(discovery([root("~/Development", { clones: 5 }), root("~/Code")]), NOW)).toBe(
      "Nothing new in ~/Development and ~/Code · looked just now",
    );
  });
});

describe("lookedLineOf", () => {
  it("adds the roots that stopped early", () => {
    expect(lookedLineOf(discovery([root("~/Code", { stopped_early: true })]), NOW)).toBe(
      "Looked just now · stopped early in ~/Code after 20 000 folders.",
    );
    expect(lookedLineOf(discovery([root("~/Code")]), NOW)).toBe("Looked just now.");
  });
});
