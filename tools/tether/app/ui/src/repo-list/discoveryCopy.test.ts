import { describe, expect, it } from "vitest";

import type { Discovery, RootInfo, Suggestion } from "../api";
import { noDiscovery, suggestion } from "../api.fake";
import {
  collapsedSummaryOf,
  errorLinesOf,
  leadOf,
  placementOf,
  rootsText,
  settledTextOf,
} from "./discoveryCopy";

const NOW = new Date("2026-11-04T12:00:00Z");
const root = (path: string, fields: Partial<RootInfo> = {}): RootInfo => ({
  path,
  clones: 0,
  stopped_early: false,
  error: null,
  ...fields,
});
const scan = (fields: Partial<Discovery>): Discovery => ({ ...noDiscovery(), ...fields });
const one: Suggestion[] = [suggestion("cache")];

describe("rootsText", () => {
  it("reads one, two, and three or more roots", () => {
    expect(rootsText([])).toBe("");
    expect(rootsText(["~/Development"])).toBe("~/Development");
    expect(rootsText(["~/Development", "~/Code"])).toBe("~/Development and ~/Code");
    expect(rootsText(["~/Development", "~/Code"], "or")).toBe("~/Development or ~/Code");
    expect(rootsText(["~/Development", "~/Code"], "comma")).toBe("~/Development, ~/Code");
    expect(rootsText(["a", "b", "c", "d"])).toBe("a, b and 2 more");
    expect(rootsText(["a", "b", "c"], "or")).toBe("a, b or 1 more");
  });
});

describe("placementOf", () => {
  const ran = { scanned_at: "2026-11-04T12:00:00Z" };

  it("nothing listed: invite, then Found as the body, then found-nothing", () => {
    expect(placementOf(noDiscovery(), false, false)).toBe("invite");
    expect(placementOf(scan({ scanning: "~/Code" }), false, false)).toBe("open");
    expect(placementOf(scan({ ...ran, suggestions: one }), false, false)).toBe("open");
    expect(placementOf(scan(ran), false, false)).toBe("found-nothing");
  });

  it("listed: nothing before the first scan", () => {
    expect(placementOf(noDiscovery(), true, false)).toBe("none");
    expect(placementOf(noDiscovery(), true, true)).toBe("none");
  });

  it("listed: open while a scan runs", () => {
    expect(placementOf(scan({ scanning: "~/Code" }), true, false)).toBe("open");
  });

  it("listed: open once this session acted, collapsed otherwise, while suggestions remain", () => {
    expect(placementOf(scan({ ...ran, suggestions: one }), true, true)).toBe("open");
    expect(placementOf(scan({ ...ran, suggestions: one }), true, false)).toBe("collapsed");
  });

  it("listed: settled when a scan ran and none remain", () => {
    expect(placementOf(scan(ran), true, false)).toBe("settled");
    expect(placementOf(scan(ran), true, true)).toBe("settled");
  });
});

describe("Found copy", () => {
  const roots = [root("~/Development", { clones: 4 }), root("~/Code", { clones: 2 })];

  it("leads with what is possible while scanning, and with where and when once done", () => {
    expect(leadOf(scan({ scanning: "~/Code" }), NOW)).toBe(
      "You can add these now; more appear as the search goes.",
    );
    expect(leadOf(scan({ roots, scanned_at: NOW.toISOString() }), NOW)).toBe(
      "In ~/Development and ~/Code, just now. Add the ones to keep on their branch.",
    );
  });

  it("has no lead once a scan finished with an unreadable root: the error line says what was searched", () => {
    const withError = [...roots, root("~/work", { clones: null, error: "permission denied" })];
    expect(leadOf(scan({ roots: withError, scanned_at: NOW.toISOString() }), NOW)).toBeUndefined();
    expect(leadOf(scan({ roots: withError, scanning: "~/work" }), NOW)).toContain("You can add these now");
  });

  it("reports an unreadable root, then which roots were searched", () => {
    const d = scan({ roots: [...roots, root("~/work", { clones: null, error: "permission denied" })] });
    expect(errorLinesOf(d)).toEqual([
      "Cannot read ~/work: permission denied. ~/Development and ~/Code were searched.",
    ]);
    expect(errorLinesOf(scan({ roots: [roots[0]!, root("~/work", { error: "no such folder." })] }))).toEqual([
      "Cannot read ~/work: no such folder. ~/Development was searched.",
    ]);
    expect(errorLinesOf(scan({ roots: [root("~/work", { error: "x" })] }))).toEqual(["Cannot read ~/work: x."]);
    expect(errorLinesOf(scan({ roots }))).toEqual([]);
  });

  it("summarises the collapsed row by when the scan ended", () => {
    expect(collapsedSummaryOf(scan({ scanned_at: "2026-11-01T12:00:00Z" }), NOW)).toBe("· looked 3 days ago");
    expect(collapsedSummaryOf(scan({}), NOW)).toBeUndefined();
  });

  it("settles on Nothing new when the roots yielded clones, else No suggestions", () => {
    const at = { scanned_at: NOW.toISOString() };
    expect(settledTextOf(scan({ ...at, roots }), NOW)).toBe("Nothing new in ~/Development, ~/Code · looked just now");
    expect(settledTextOf(scan({ ...at, roots: [root("~/Development"), root("~/Code")] }), NOW)).toBe(
      "No suggestions · looked just now",
    );
  });
});
