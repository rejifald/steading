import { describe, expect, it } from "vitest";

import { shownSize, totalSizeOf } from "./shownSize";

describe("shownSize", () => {
  it("shows bytes in decimal units, one decimal below ten", () => {
    expect(shownSize(512)).toBe("512 bytes");
    expect(shownSize(840_000_000)).toBe("840 MB");
    expect(shownSize(3_240_000_000)).toBe("3.2 GB");
    expect(shownSize(14_400_000_000)).toBe("14 GB");
  });
});

describe("totalSizeOf", () => {
  it("adds the measured worktrees and skips the unmeasured ones", () => {
    expect(totalSizeOf([{ size_bytes: 2_000 }, {}, { size_bytes: 3_000 }])).toBe(5_000);
  });
});
