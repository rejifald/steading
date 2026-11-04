import { render } from "@testing-library/react";
import type { ComponentType, SVGProps } from "react";
import { describe, expect, it } from "vitest";

import * as icons from "./icons";

const ICONS = icons as Record<string, ComponentType<SVGProps<SVGSVGElement>>>;

describe("icons", () => {
  it.each(Object.entries(ICONS))("%s is hidden from assistive technology", (_, Icon) => {
    const { container } = render(<Icon />);
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });
});
