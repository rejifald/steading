import { Button } from "@base-ui/react/button";
import { Collapsible } from "@base-ui/react/collapsible";
import { Children, type ReactNode, useState } from "react";

import { ChevronRightIcon } from "@steading/tray-popover/controls/icons";
import { DOT_COLOURS } from "../StateLabel";
import { type Outcome, OUTCOME_TONES } from "./outcomeOf";

interface OutcomeSectionProps {
  outcome: Outcome;
  title: string;
  count: number;
  /** The disk the section's worktrees take, shown after the count of an open section. */
  size?: string | undefined;
  /** One sentence under the heading, or what replaces it (the Remove all confirm). */
  lead?: ReactNode;
  /** Sits at the right of the heading. */
  action?: ReactNode;
  /** Draws a closed one-line section that opens on click. */
  collapsible?: boolean;
  /** The right side of a collapsed line. */
  summary?: string;
  /** Rows shown before "Show N more". */
  limit?: number;
  children: ReactNode;
}

const FOCUS_RING = "focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus";

function Dot({ outcome }: { outcome: Outcome }) {
  return <span aria-hidden className={`size-2 shrink-0 rounded-full ${DOT_COLOURS[OUTCOME_TONES[outcome]]}`} />;
}

/** The rows of a section as a list, cut to `limit` until "Show N more" is clicked. */
function Rows({ title, limit, children }: { title: string; limit: number | undefined; children: ReactNode }) {
  const [isExpanded, setIsExpanded] = useState(false);
  const rows = Children.toArray(children);
  const hiddenCount = limit === undefined || isExpanded ? 0 : Math.max(0, rows.length - limit);
  const visibleRows = hiddenCount > 0 ? rows.slice(0, limit) : rows;
  return (
    <>
      <ul aria-label={title}>{visibleRows}</ul>
      {hiddenCount > 0 && (
        <Button
          onClick={() => setIsExpanded(true)}
          className={`flex h-8 w-full items-center px-4 text-sm text-muted hover:bg-surface-hover ${FOCUS_RING}`}
        >
          {`Show ${hiddenCount} more`}
        </Button>
      )}
    </>
  );
}

/** One outcome's rows under a heading: open, or a collapsed line that expands. */
export function OutcomeSection({
  outcome,
  title,
  count,
  size,
  lead,
  action,
  collapsible = false,
  summary,
  limit,
  children,
}: OutcomeSectionProps) {
  if (count === 0) return null;
  if (collapsible) {
    return (
      <section aria-label={title}>
        <Collapsible.Root>
          <Collapsible.Trigger
            className={`group flex min-h-8 w-full flex-wrap items-center gap-x-2 border-t border-border px-4 py-1.5 text-sm hover:bg-surface-hover ${FOCUS_RING}`}
          >
            <ChevronRightIcon className="shrink-0 text-muted group-data-panel-open:rotate-90" />
            <Dot outcome={outcome} />
            <span className="font-semibold whitespace-nowrap text-text">{title}</span>
            <span className="text-faint">{count}</span>
            {/* Wraps under the title when it does not fit beside it, still right-aligned. */}
            {summary && <span className="ml-auto text-right text-xs text-faint">{summary}</span>}
          </Collapsible.Trigger>
          <Collapsible.Panel>
            <Rows title={title} limit={limit}>
              {children}
            </Rows>
          </Collapsible.Panel>
        </Collapsible.Root>
      </section>
    );
  }
  return (
    <section aria-label={title}>
      <div className="flex items-center gap-2 px-4 pt-3 pb-1">
        <h2 className="flex items-center gap-2 text-sm font-semibold text-text">
          <Dot outcome={outcome} />
          {title}
          <span className="font-normal text-faint">{size ? `${count} · ${size}` : count}</span>
        </h2>
        {action && <span className="ml-auto">{action}</span>}
      </div>
      {lead && <div className="px-4 pb-1 text-xs text-muted">{lead}</div>}
      <Rows title={title} limit={limit}>
        {children}
      </Rows>
    </section>
  );
}
