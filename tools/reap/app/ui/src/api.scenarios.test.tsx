import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { createFakeApi } from "./api.fake";
import { SCENARIOS } from "./api.scenarios";
import { Popover } from "./Popover";

describe("SCENARIOS", () => {
  it.each(Object.entries(SCENARIOS))("%s renders the popover", async (_, options) => {
    render(<Popover api={createFakeApi(options)} />);
    expect(await screen.findByRole("banner")).toBeInTheDocument();
  });
});
