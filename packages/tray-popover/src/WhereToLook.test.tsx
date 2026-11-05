import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { WhereToLook, WhereToLookFooter } from "./WhereToLook";

type Props = Parameters<typeof WhereToLook>[0];

function view(props: Partial<Props> = {}) {
  const handlers = {
    onAddRoot: vi.fn(),
    onRemoveRoot: vi.fn(),
    onUnskip: vi.fn(),
  };
  render(
    <WhereToLook
      foldersTitle="Folders"
      roots={[
        { path: "~/Development", found: "5 clones" },
        { path: "~/Code", found: "3 clones" },
      ]}
      addFolderLabel="Add folder…"
      removeRootLabel={(path) => `Stop looking in ${path}`}
      note="Desktop, Documents and Downloads are searched only if you add them here."
      skippedTitle="Skipped"
      skipped={[
        { path: "~/Development/scratch", name: "scratch", root: "~/Development" },
        { path: "~/Code/old-site", name: "old-site", root: "~/Code" },
      ]}
      unskipLabel="Unskip"
      {...handlers}
      {...props}
    />,
  );
  return handlers;
}

describe("WhereToLook", () => {
  it("lists the folders and the skipped clones under two labelled headings", () => {
    view();
    const folders = screen.getByRole("list", { name: "Folders" });
    expect(within(folders).getByText("~/Development")).toHaveClass("flex-1", "font-medium");
    expect(within(folders).getByText("5 clones")).toHaveClass("text-xs", "text-faint");
    const skipped = screen.getByRole("list", { name: "Skipped" });
    expect(within(skipped).getByText("scratch")).toHaveClass("font-medium");
    expect(within(skipped).getByText("~/Development")).toHaveClass("flex-1", "text-faint");
    expect(screen.getByText(/Desktop, Documents/)).toHaveClass("px-4", "pb-2", "text-xs", "text-muted");
  });

  it("calls back to add a folder", async () => {
    const user = userEvent.setup();
    const { onAddRoot } = view();
    await user.click(screen.getByRole("button", { name: "Add folder…" }));
    expect(onAddRoot).toHaveBeenCalledTimes(1);
  });

  it("starts focus on the first control, Add folder…", () => {
    view();
    expect(screen.getByRole("button", { name: "Add folder…" })).toHaveFocus();
  });

  it("removes a root by its path", async () => {
    const user = userEvent.setup();
    const { onRemoveRoot } = view();
    await user.click(screen.getByRole("button", { name: "Stop looking in ~/Code" }));
    expect(onRemoveRoot).toHaveBeenCalledExactlyOnceWith("~/Code");
  });

  it("unskips a clone by its path", async () => {
    const user = userEvent.setup();
    const { onUnskip } = view();
    const unskip = screen.getAllByRole("button", { name: "Unskip" });
    expect(unskip[1]).toHaveAccessibleDescription("old-site");
    await user.click(unskip[1]!);
    expect(onUnskip).toHaveBeenCalledExactlyOnceWith("~/Code/old-site");
  });

  it("hides Skipped while nothing is skipped", () => {
    view({ skipped: [] });
    expect(screen.queryByRole("heading", { name: /Skipped/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Unskip" })).not.toBeInTheDocument();
    expect(screen.getAllByRole("list")).toHaveLength(1);
  });

  it("shows a root's error in place of its count, in the needs-you colour", () => {
    view({
      roots: [
        { path: "~/Development", found: "5 clones" },
        { path: "~/work", error: "not allowed to read it" },
      ],
    });
    expect(screen.getByText("not allowed to read it")).toHaveClass("text-needs-you");
    expect(screen.queryByText("3 clones")).not.toBeInTheDocument();
  });

  it("shows no count for a root that has not been scanned", () => {
    view({ roots: [{ path: "~/Development" }] });
    const item = within(screen.getByRole("list", { name: "Folders" })).getByText("~/Development").closest("li");
    expect(within(item as HTMLElement).queryByText(/clone/)).not.toBeInTheDocument();
  });

  it("leaves only Add folder… when the last root is removed", () => {
    view({ roots: [] });
    expect(screen.getByRole("heading", { name: "Folders" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add folder…" })).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Folders" })).not.toBeInTheDocument();
  });

  it("truncates a long path and carries it as the title", () => {
    const path = "~/Development/a/very/deeply/nested/folder/that/does/not/fit";
    view({ roots: [{ path, found: "1 clone" }] });
    expect(screen.getByText(path)).toHaveClass("truncate");
    expect(screen.getByText(path)).toHaveAttribute("title", path);
  });
});

describe("WhereToLookFooter", () => {
  function footer(props: Partial<Parameters<typeof WhereToLookFooter>[0]> = {}) {
    const onDone = vi.fn();
    const onLookAgain = vi.fn();
    render(
      <WhereToLookFooter
        doneLabel="Done"
        onDone={onDone}
        lookAgainLabel="Look again"
        onLookAgain={onLookAgain}
        {...props}
      />,
    );
    return { onDone, onLookAgain };
  }

  it("calls back for Done and for Look again", async () => {
    const user = userEvent.setup();
    const { onDone, onLookAgain } = footer();
    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(onDone).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "Look again" }));
    expect(onLookAgain).toHaveBeenCalledTimes(1);
  });

  it("puts Done quiet at the left and Look again primary at the right", () => {
    footer();
    expect(screen.getByRole("button", { name: "Done" })).toHaveClass("bg-surface-raised");
    expect(screen.getByRole("button", { name: "Look again" })).toHaveClass("bg-accent", "ml-auto");
  });

  it("disables Look again with no folder to look in", async () => {
    const user = userEvent.setup();
    const { onLookAgain } = footer({ lookAgainDisabled: true });
    const lookAgain = screen.getByRole("button", { name: "Look again" });
    expect(lookAgain).toBeDisabled();
    await user.click(lookAgain);
    expect(onLookAgain).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Done" })).toBeEnabled();
  });
});
