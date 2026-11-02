import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { Row } from "../api";
import { DISMISSED_KEY, useDismissed } from "./useDismissed";

const PATH = "~/code/example-site";
const REASON = "on another branch (feature/new-header)";

const site = (health: Row["health"]): Row => ({
  name: "example-site",
  path: PATH,
  branch: "release",
  mode: "follow",
  remote: "origin",
  interval_seconds: 300,
  health,
});

function setup(rows: Row[] = [site("needs-you")], details: Record<string, string> = { [PATH]: REASON }) {
  return renderHook(({ rows, details }) => useDismissed(rows, details), {
    initialProps: { rows, details },
  });
}

describe("useDismissed", () => {
  it("hides a dismissed callout and remembers it in storage", () => {
    const { result } = setup();
    expect(result.current.isDismissed(PATH)).toBe(false);
    act(() => result.current.dismiss(PATH));
    expect(result.current.isDismissed(PATH)).toBe(true);
    expect(JSON.parse(localStorage.getItem(DISMISSED_KEY) ?? "{}")).toEqual({ [PATH]: REASON });
  });

  it("keeps the dismissal across a restart", () => {
    localStorage.setItem(DISMISSED_KEY, JSON.stringify({ [PATH]: REASON }));
    const { result } = setup();
    expect(result.current.isDismissed(PATH)).toBe(true);
  });

  it("shows the callout again when the reason changes", () => {
    const { result, rerender } = setup();
    act(() => result.current.dismiss(PATH));
    rerender({ rows: [site("needs-you")], details: { [PATH]: "HEAD is detached" } });
    expect(result.current.isDismissed(PATH)).toBe(false);
    rerender({ rows: [site("needs-you")], details: { [PATH]: REASON } });
    expect(result.current.isDismissed(PATH)).toBe(false);
  });

  it("forgets the dismissal once the repo stops needing you", () => {
    const { result, rerender } = setup();
    act(() => result.current.dismiss(PATH));
    rerender({ rows: [site("in-sync")], details: {} });
    rerender({ rows: [site("needs-you")], details: { [PATH]: REASON } });
    expect(result.current.isDismissed(PATH)).toBe(false);
  });

  it("forgets the dismissal of a repo no longer listed", () => {
    const { result, rerender } = setup();
    act(() => result.current.dismiss(PATH));
    rerender({ rows: [], details: {} });
    expect(localStorage.getItem(DISMISSED_KEY)).toBe("{}");
  });

  it("keeps every dismissal while there is no report", () => {
    const { result, rerender } = renderHook(({ rows }) => useDismissed(rows, { [PATH]: REASON }), {
      initialProps: { rows: [site("needs-you")] as Row[] | null },
    });
    act(() => result.current.dismiss(PATH));
    rerender({ rows: null });
    rerender({ rows: [site("needs-you")] });
    expect(result.current.isDismissed(PATH)).toBe(true);
  });

  it.each(["not json", "[1, 2]", '{"~/code/x": 3}'])("ignores corrupt storage: %s", (stored) => {
    localStorage.setItem(DISMISSED_KEY, stored);
    const { result } = setup();
    expect(result.current.isDismissed(PATH)).toBe(false);
  });
});
