import { describe, expect, it } from "vitest";

import { noticeOf } from "./noticeOf";

describe("noticeOf", () => {
  it("shows a failed action verbatim, before the app's note", () => {
    expect(noticeOf({ actionError: "git fetch: exit status 128", note: "pause failed", staleError: null })).toBe(
      "git fetch: exit status 128",
    );
  });

  it("shows the app's note", () => {
    expect(noticeOf({ actionError: null, note: "pause failed", staleError: null })).toBe("pause failed");
  });

  it("says the last report is shown when repos.toml cannot be read", () => {
    expect(noticeOf({ actionError: null, note: null, staleError: "expected `=` at line 4" })).toBe(
      "Cannot read repos.toml: expected `=` at line 4. Showing the last report.",
    );
  });

  it("is null when all is well", () => {
    expect(noticeOf({ actionError: null, note: null, staleError: null })).toBeNull();
  });
});
