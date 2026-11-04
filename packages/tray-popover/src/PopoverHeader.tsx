import type { ReactNode } from "react";

import { ProgressLine } from "./ProgressLine";

interface PopoverHeaderProps {
  /** The tool's name, or what the view is about (a repo's name in its settings); truncates. */
  title: string;
  /** The overall state beside the title, announced as a status. */
  state?: ReactNode;
  /** Pushed to the right: a quiet summary or a state. */
  aside?: ReactNode;
  /** Draws the progress hairline on the bottom edge while something runs. */
  isPending?: boolean;
}

/** The popover's top band: title, state and a right slot, above a border that never scrolls. */
export function PopoverHeader({ title, state, aside, isPending = false }: PopoverHeaderProps) {
  return (
    <header className="relative flex items-center gap-2 border-b border-border px-4 py-3">
      <h1 className="min-w-0 truncate text-base font-semibold text-text">{title}</h1>
      {state !== undefined && (
        <span className="text-sm" role="status">
          {state}
        </span>
      )}
      {aside !== undefined && aside !== null && <span className="ml-auto shrink-0">{aside}</span>}
      {isPending && <ProgressLine />}
    </header>
  );
}
