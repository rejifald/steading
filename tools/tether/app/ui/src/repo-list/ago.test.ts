import { describe, expect, it } from "vitest";

import { ago } from "./ago";

describe("ago", () => {
  const now = new Date("2026-01-01T12:00:00Z");
  const before = (seconds: number) => new Date(now.getTime() - seconds * 1000).toISOString();

  it("reads naturally at each scale", () => {
    expect(ago(undefined, now)).toBe("never");
    expect(ago(before(5), now)).toBe("just now");
    expect(ago(before(180), now)).toBe("3 min ago");
    expect(ago(before(7200), now)).toBe("2 h ago");
    expect(ago(before(3 * 86_400), now)).toBe("3 d ago");
  });

  it("treats a time in the future as just now", () => {
    expect(ago(before(-60), now)).toBe("just now");
  });
});
