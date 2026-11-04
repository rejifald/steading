import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { TextButton } from "./TextButton";

describe("TextButton", () => {
  it("puts the inset variant on bg-surface", () => {
    render(<TextButton variant="inset">Inset</TextButton>);
    const classes = screen.getByRole("button", { name: "Inset" }).className.split(" ");
    expect(classes).toContain("bg-surface");
    expect(classes).not.toContain("bg-surface-raised");
  });

  it("keeps the quiet variant on bg-surface-raised", () => {
    render(<TextButton>Quiet</TextButton>);
    const classes = screen.getByRole("button", { name: "Quiet" }).className.split(" ");
    expect(classes).toContain("bg-surface-raised");
    expect(classes).not.toContain("bg-surface");
  });

  it("is shorter and lighter at the compact size", () => {
    render(<TextButton size="compact">Compact</TextButton>);
    const classes = screen.getByRole("button", { name: "Compact" }).className.split(" ");
    expect(classes).toEqual(expect.arrayContaining(["h-6", "font-normal"]));
  });
});
