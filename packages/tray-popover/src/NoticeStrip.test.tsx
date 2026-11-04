import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { NoticeStrip } from "./NoticeStrip";

describe("NoticeStrip", () => {
  it("shows the text verbatim as an alert", () => {
    render(<NoticeStrip text="git fetch: exit status 128" />);
    expect(screen.getByRole("alert")).toHaveTextContent(/^git fetch: exit status 128$/);
  });

  it("renders nothing without text", () => {
    const { container } = render(<NoticeStrip text={null} />);
    expect(container).toBeEmptyDOMElement();
  });
});
