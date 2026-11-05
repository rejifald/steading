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

  describe("link variant", () => {
    it("renders a real button named by its text", () => {
      render(
        <TextButton variant="link" size="compact">
          Where to look…
        </TextButton>,
      );
      expect(screen.getByRole("button", { name: "Where to look…" })).toBeInTheDocument();
    });

    it("is underlined text with no box", () => {
      render(
        <TextButton variant="link" size="compact">
          Link
        </TextButton>,
      );
      const button = screen.getByRole("button", { name: "Link" });
      const classes = button.className.split(" ");
      expect(classes).toEqual(
        expect.arrayContaining(["bg-transparent", "text-text", "text-xs", "underline", "px-2", "h-6", "rounded-control"]),
      );
      expect(classes).not.toContain("border");
      expect(classes).not.toContain("border-border");
    });

    it("keeps the underline and gains no background on hover", () => {
      render(
        <TextButton variant="link" size="compact">
          Link
        </TextButton>,
      );
      const classes = screen.getByRole("button", { name: "Link" }).className.split(" ");
      expect(classes).not.toContain("no-underline");
      expect(classes.filter((c) => c.startsWith("hover:"))).toEqual([]);
    });

    it("overrides the base text size and padding instead of fighting them", () => {
      render(
        <TextButton variant="link" size="compact">
          Link
        </TextButton>,
      );
      const classes = screen.getByRole("button", { name: "Link" }).className.split(" ");
      expect(classes).not.toContain("text-sm");
      expect(classes).not.toContain("px-3");
    });

    it("keeps the focus outline and dims when disabled", () => {
      render(
        <TextButton variant="link" size="compact" disabled>
          Link
        </TextButton>,
      );
      const button = screen.getByRole("button", { name: "Link" });
      expect(button).toBeDisabled();
      expect(button).toHaveClass("focus-visible:outline-focus", "disabled:opacity-50");
    });
  });

  it("leaves the existing variants on 13px text with 12px padding", () => {
    render(
      <>
        <TextButton variant="primary">Primary</TextButton>
        <TextButton variant="quiet">Quiet</TextButton>
        <TextButton variant="inset" size="compact">
          Inset
        </TextButton>
      </>,
    );
    for (const name of ["Primary", "Quiet", "Inset"]) {
      const classes = screen.getByRole("button", { name }).className.split(" ");
      expect(classes).toEqual(expect.arrayContaining(["text-sm", "px-3"]));
      expect(classes).not.toContain("underline");
    }
    expect(screen.getByRole("button", { name: "Primary" })).toHaveClass("bg-accent", "text-accent-text");
  });
});
