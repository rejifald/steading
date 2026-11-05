import { describe, expect, it } from "vitest";

import type { Discovery, Suggestion } from "../api";
import { foundModeOf } from "./foundMode";

const suggestion = (name: string): Suggestion => ({
  path: `~/Development/${name}`,
  name,
  root: "~/Development",
  worktrees: 2,
});

function discovery(fields: Partial<Discovery> = {}): Discovery {
  return { scanned_at: null, scanning: null, roots: [], suggestions: [], skipped: [], ...fields };
}

const WHEN = "2026-01-01T12:00:00Z";

describe("foundModeOf with nothing listed", () => {
  const modeOf = (fields: Partial<Discovery>) =>
    foundModeOf({ discovery: discovery(fields), hasRepos: false, isEngaged: false });

  it("is none before a scan, so the invitation shows", () => {
    expect(modeOf({})).toBeNull();
  });

  it("is open as the whole body while a scan runs or suggestions wait", () => {
    expect(modeOf({ scanning: "~/Code" })).toBe("open");
    expect(modeOf({ scanned_at: WHEN, suggestions: [suggestion("api")] })).toBe("open");
  });

  it("is none after a scan found nothing, so found-nothing shows", () => {
    expect(modeOf({ scanned_at: WHEN })).toBeNull();
  });
});

describe("foundModeOf with repos listed", () => {
  const modeOf = (fields: Partial<Discovery>, isEngaged = false) =>
    foundModeOf({ discovery: discovery(fields), hasRepos: true, isEngaged });

  it("is nothing at all before the first scan", () => {
    expect(modeOf({})).toBeNull();
  });

  it("is open while a scan runs, engaged or not", () => {
    expect(modeOf({ scanning: "~/Code" })).toBe("open");
    expect(modeOf({ scanning: "~/Code", suggestions: [suggestion("api")] })).toBe("open");
  });

  it("is collapsed while suggestions remain and this session has not worked on them", () => {
    expect(modeOf({ scanned_at: WHEN, suggestions: [suggestion("api")] })).toBe("collapsed");
  });

  it("is open once this session has scanned or answered a suggestion", () => {
    expect(modeOf({ scanned_at: WHEN, suggestions: [suggestion("api")] }, true)).toBe("open");
  });

  it("settles when a scan ran and none remain", () => {
    expect(modeOf({ scanned_at: WHEN })).toBe("settled");
    expect(modeOf({ scanned_at: WHEN }, true)).toBe("settled");
  });
});
