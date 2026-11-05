import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { FoundPreview, FoundRow, FoundSection } from "./FoundSection";

interface RowProps {
  name?: string;
  onAdd?: () => void;
  onSkip?: () => void;
  busy?: boolean;
}

function row({ name = "api", onAdd = () => {}, onSkip = () => {}, busy }: RowProps = {}) {
  return (
    <FoundRow
      key={name}
      name={name}
      detail="main · ~/Development"
      title={`~/Development/${name}`}
      onAdd={onAdd}
      addLabel="Add"
      onSkip={onSkip}
      skipLabel={`Skip ${name}`}
      {...(busy === undefined ? {} : { busy })}
    />
  );
}

describe("FoundSection open", () => {
  it("is a section labelled by its heading, with the count and the rows", () => {
    render(
      <FoundSection mode="open" title="Found" count={2} lead="Add the ones to keep.">
        {row({ name: "api" })}
        {row({ name: "web" })}
      </FoundSection>,
    );
    const section = screen.getByRole("region", { name: "Found" });
    expect(within(section).getByRole("heading", { level: 2, name: "Found 2" })).toBeInTheDocument();
    expect(within(section).getByText("Add the ones to keep.")).toHaveClass("text-xs", "text-muted");
    expect(within(section).getByRole("list", { name: "Found" }).children).toHaveLength(2);
  });

  it("draws the aside when nothing is scanning", () => {
    render(
      <FoundSection mode="open" title="Found" count={1} aside={<button type="button">Where to look…</button>}>
        {row()}
      </FoundSection>,
    );
    expect(screen.getByRole("button", { name: "Where to look…" })).toBeInTheDocument();
  });

  it("shows the scanning status in a polite live region, in place of the aside", () => {
    render(
      <FoundSection
        mode="open"
        title="Found"
        count={2}
        scanning="looking in ~/Code…"
        aside={<button type="button">Where to look…</button>}
        ghostRows={3}
      >
        {row()}
      </FoundSection>,
    );
    const status = screen.getByRole("status");
    expect(status).toHaveTextContent("looking in ~/Code…");
    expect(status).toHaveAttribute("aria-live", "polite");
    expect(status).toHaveClass("text-xs", "text-progress");
    expect(status.querySelector("svg")).toHaveClass("animate-spin");
    expect(screen.queryByRole("button", { name: "Where to look…" })).not.toBeInTheDocument();
  });

  it("keeps the live region in the tree before a scan starts, so the first text is announced", () => {
    const { rerender } = render(<FoundSection mode="open" title="Found" count={0} />);
    expect(screen.getByRole("status")).toBeEmptyDOMElement();
    rerender(<FoundSection mode="open" title="Found" count={0} scanning="looking in ~/Code…" />);
    expect(screen.getByRole("status")).toHaveTextContent("looking in ~/Code…");
  });

  it("hides the ghost rows from the accessibility tree", () => {
    const { container } = render(
      <FoundSection mode="open" title="Found" count={1} scanning="looking in ~/Code…" ghostRows={3}>
        {row()}
      </FoundSection>,
    );
    const ghosts = container.querySelector('ul[aria-hidden="true"]');
    expect(ghosts).toHaveClass("opacity-50");
    expect(ghosts?.children).toHaveLength(3);
    expect(screen.getAllByRole("list")).toHaveLength(1);
    expect(screen.getByRole("list")).toHaveAccessibleName("Found");
  });

  it("draws no ghost list without ghost rows", () => {
    const { container } = render(<FoundSection mode="open" title="Found" count={1}>{row()}</FoundSection>);
    expect(container.querySelector('ul[aria-hidden="true"]')).toBeNull();
  });

  it("draws one warning line per unreadable root", () => {
    render(
      <FoundSection mode="open" title="Found" count={1} errors={["Cannot read ~/work.", "Cannot read ~/src."]}>
        {row()}
      </FoundSection>,
    );
    const line = screen.getByText("Cannot read ~/work.");
    expect(line).toHaveClass("text-xs", "text-text");
    expect(line.querySelector("svg")).toHaveClass("text-needs-you");
    expect(screen.getByText("Cannot read ~/src.")).toBeInTheDocument();
  });

  it("draws the rule above only when listed rows sit above it", () => {
    const { container, rerender } = render(<FoundSection mode="open" title="Found" count={1}>{row()}</FoundSection>);
    const heading = () => container.querySelector("h2")?.parentElement;
    expect(heading()).toHaveClass("border-t", "border-transparent");
    rerender(
      <FoundSection mode="open" title="Found" count={1} hasRuleAbove>
        {row()}
      </FoundSection>,
    );
    expect(heading()).toHaveClass("border-t", "border-border");
  });
});

describe("FoundRow", () => {
  it("calls back for Add and for the cross", async () => {
    const user = userEvent.setup();
    const onAdd = vi.fn();
    const onSkip = vi.fn();
    render(
      <ul>
        {row({ name: "api", onAdd, onSkip })}
        {row({ name: "web" })}
      </ul>,
    );
    await user.click(screen.getAllByRole("button", { name: "Add" })[0]!);
    expect(onAdd).toHaveBeenCalledTimes(1);
    expect(onSkip).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Skip api" }));
    expect(onSkip).toHaveBeenCalledTimes(1);
  });

  it("tabs Add, then the cross, per row, top to bottom", async () => {
    const user = userEvent.setup();
    render(
      <ul>
        {row({ name: "api" })}
        {row({ name: "web" })}
      </ul>,
    );
    await user.tab();
    expect(screen.getAllByRole("button", { name: "Add" })[0]).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Skip api" })).toHaveFocus();
    await user.tab();
    expect(screen.getAllByRole("button", { name: "Add" })[1]).toHaveFocus();
  });

  it("truncates a long name and detail, and carries the full path as the title", () => {
    render(
      <ul>
        <FoundRow
          name="a-very-long-clone-folder-name-that-will-not-fit"
          detail="main · ~/Development/deeply/nested/place"
          title="~/Development/deeply/nested/place/a-very-long-clone-folder-name-that-will-not-fit"
          onAdd={() => {}}
          addLabel="Add"
          onSkip={() => {}}
          skipLabel="Skip it"
        />
      </ul>,
    );
    const title = "~/Development/deeply/nested/place/a-very-long-clone-folder-name-that-will-not-fit";
    const name = screen.getByText("a-very-long-clone-folder-name-that-will-not-fit");
    const detail = screen.getByText("main · ~/Development/deeply/nested/place");
    expect(name).toHaveClass("truncate", "font-medium");
    expect(name).toHaveAttribute("title", title);
    expect(detail).toHaveClass("truncate", "flex-1", "text-faint");
    expect(detail).toHaveAttribute("title", title);
  });

  it("disables Add while an add is in flight, and the row stays", () => {
    render(<ul>{row({ busy: true })}</ul>);
    expect(screen.getByRole("button", { name: "Add" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Add" })).toHaveAttribute("aria-busy", "true");
    expect(screen.getByText("api")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Skip api" })).toBeEnabled();
  });

  it("describes Add by the clone's name", () => {
    render(<ul>{row({ name: "api" })}</ul>);
    expect(screen.getByRole("button", { name: "Add" })).toHaveAccessibleDescription("api");
  });
});

describe("FoundSection collapsed", () => {
  function collapsed(onLookAgain = () => {}) {
    return (
      <FoundSection
        mode="collapsed"
        title="Found"
        count={2}
        collapsedSummary="· looked 3 days ago"
        onLookAgain={onLookAgain}
        lookAgainLabel="Look again"
      >
        {row({ name: "api" })}
        {row({ name: "web" })}
      </FoundSection>
    );
  }

  it("is one line: toggle with title, count and summary, and Look again", () => {
    render(collapsed());
    const toggle = screen.getByRole("button", { name: /Found/ });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(toggle).toHaveAccessibleName("Found 2 · looked 3 days ago");
    expect(screen.getByRole("button", { name: "Look again" })).toBeInTheDocument();
    expect(screen.queryByText("api")).not.toBeInTheDocument();
  });

  it("shows and hides the rows from the toggle", async () => {
    const user = userEvent.setup();
    render(collapsed());
    const toggle = screen.getByRole("button", { name: /Found/ });
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("api")).toBeInTheDocument();
    expect(screen.getByText("web")).toBeInTheDocument();
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("api")).not.toBeInTheDocument();
  });

  it("fires Look again without toggling, and the button is not inside the toggle", async () => {
    const user = userEvent.setup();
    const onLookAgain = vi.fn();
    render(collapsed(onLookAgain));
    const toggle = screen.getByRole("button", { name: /Found/ });
    const lookAgain = screen.getByRole("button", { name: "Look again" });
    expect(toggle).not.toContainElement(lookAgain);
    await user.click(lookAgain);
    expect(onLookAgain).toHaveBeenCalledTimes(1);
    expect(toggle).toHaveAttribute("aria-expanded", "false");
  });

  it("has the rule above it", () => {
    const { container } = render(collapsed());
    expect(container.querySelector(".border-t.border-border")).not.toBeNull();
  });
});

describe("FoundSection settled", () => {
  it("shows the text and Look again, and no toggle or rows", async () => {
    const user = userEvent.setup();
    const onLookAgain = vi.fn();
    const { container } = render(
      <FoundSection
        mode="settled"
        title="Found"
        count={0}
        settledText="No suggestions · looked just now"
        onLookAgain={onLookAgain}
        lookAgainLabel="Look again"
      />,
    );
    expect(screen.getByText("No suggestions · looked just now")).toHaveClass("flex-1", "text-xs", "text-faint");
    expect(container.querySelector("svg")).toHaveClass("text-faint");
    expect(screen.queryByRole("heading")).not.toBeInTheDocument();
    expect(screen.getAllByRole("button")).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "Look again" }));
    expect(onLookAgain).toHaveBeenCalledTimes(1);
  });

  it("draws no button without a handler", () => {
    render(<FoundSection mode="settled" title="Found" count={0} settledText="Nothing new" />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});

describe("FoundPreview", () => {
  it("is a dashed, faded box hidden from assistive tech, with Add pills that are not buttons", () => {
    const { container } = render(
      <FoundPreview
        rows={[
          { name: "your-app", detail: "main · ~/Development" },
          { name: "your-docs", detail: "main · ~/Code" },
        ]}
        addLabel="Add"
      />,
    );
    const box = container.firstElementChild;
    expect(box).toHaveAttribute("aria-hidden", "true");
    expect(box).toHaveClass("border-dashed", "opacity-50");
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
    expect(box?.children).toHaveLength(2);
    expect(within(box as HTMLElement).getAllByText("Add")).toHaveLength(2);
    expect(within(box as HTMLElement).getByText("your-docs")).toBeInTheDocument();
  });
});
