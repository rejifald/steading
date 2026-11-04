import type { ReactNode } from "react";

import type { State } from "../api";
import { STATE_LABELS, StateLabel } from "../StateLabel";
import { outcomeOf } from "./outcomeOf";

interface WorktreeLineProps {
  name: string;
  /** The faint text after the name: branch and pull request, or a path. */
  detail: string;
  /** Omitted for a line that has no state yet, such as the add-form preview. */
  state?: State;
  /** Draws the line as a placeholder hidden from assistive technology. */
  ghost?: boolean;
  /** Sits between the detail and the state column (the row's hover actions). */
  children?: ReactNode;
}

/** One display-only line for a worktree: name, detail, optional actions and state. */
export function WorktreeLine({ name, detail, state, ghost = false, children }: WorktreeLineProps) {
  return (
    <div aria-hidden={ghost || undefined} className="flex h-8 items-center gap-2 px-2">
      <span className="truncate font-medium">{name}</span>
      <span className="flex-1 truncate text-faint">{detail}</span>
      {children}
      <span className="w-22 text-xs text-muted">
        {state ? <StateLabel glyph={outcomeOf(state)} label={STATE_LABELS[state]} /> : null}
      </span>
    </div>
  );
}
