import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Callout } from "./Callout";

describe("Callout", () => {
  it("tints the top strip in the needs-you tone", () => {
    render(<Callout tone="needs-you" heading="Not added" />);
    expect(screen.getByText("Not added")).toHaveClass("bg-needs-you/14", "border-needs-you/30");
  });

  it("leaves the strip untinted in the plain tone", () => {
    render(<Callout tone="plain" heading="Stop keeping it?" />);
    expect(screen.getByText("Stop keeping it?")).toHaveClass("bg-surface-raised", "border-border");
  });

  it("is raised, and never shrinks inside a scrolling column", () => {
    const { container } = render(<Callout tone="plain" heading="x" />);
    expect(container.firstElementChild).toHaveClass("shadow-raised", "shrink-0");
  });

  it("passes its role and label through", () => {
    render(
      <>
        <Callout tone="needs-you" role="alert" heading="Not saved" />
        <Callout tone="needs-you" aria-label="example-site needs you" heading="x" />
      </>,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("Not saved");
    expect(screen.getByRole("region", { name: "example-site needs you" })).toBeInTheDocument();
  });

  it("draws no body without children", () => {
    const { container } = render(<Callout tone="plain" heading="x" />);
    expect(container.firstElementChild?.children).toHaveLength(1);
  });
});
