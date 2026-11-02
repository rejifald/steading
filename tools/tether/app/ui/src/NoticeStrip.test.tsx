import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { NoticeStrip } from "./NoticeStrip";

describe("NoticeStrip", () => {
  it("shows a failed action verbatim, before the app's note", () => {
    render(<NoticeStrip actionError="git fetch: exit status 128" note="pause failed" staleError={null} />);
    expect(screen.getByRole("alert")).toHaveTextContent(/^git fetch: exit status 128$/);
  });

  it("shows the app's note", () => {
    render(<NoticeStrip actionError={null} note="pause failed" staleError={null} />);
    expect(screen.getByRole("alert")).toHaveTextContent("pause failed");
  });

  it("says the last report is shown when repos.toml cannot be read", () => {
    render(<NoticeStrip actionError={null} note={null} staleError="expected `=` at line 4" />);
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Cannot read repos.toml: expected `=` at line 4. Showing the last report.",
    );
  });

  it("renders nothing when all is well", () => {
    const { container } = render(<NoticeStrip actionError={null} note={null} staleError={null} />);
    expect(container).toBeEmptyDOMElement();
  });
});
