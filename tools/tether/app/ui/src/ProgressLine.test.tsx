import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ProgressLine } from "./ProgressLine";

describe("ProgressLine", () => {
  it("is hidden from the accessibility tree", () => {
    const { container } = render(<ProgressLine />);
    expect(container.firstElementChild).toHaveAttribute("aria-hidden", "true");
  });

  it("sits inside a row's padding when inset, and on the full width otherwise", () => {
    const { container, rerender } = render(<ProgressLine inset />);
    expect(container.firstElementChild).toHaveClass("inset-x-2");
    rerender(<ProgressLine />);
    expect(container.firstElementChild).toHaveClass("inset-x-0");
  });

  it("stops the sweep under reduced motion", () => {
    const { container } = render(<ProgressLine />);
    expect(container.querySelector(".animate-progress")).toHaveClass("motion-reduce:animate-none");
  });
});
