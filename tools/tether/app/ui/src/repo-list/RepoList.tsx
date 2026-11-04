import { ScrollArea } from "@base-ui/react/scroll-area";
import type { ReactNode } from "react";

import type { Row } from "../api";
import { CalloutStack } from "./CalloutStack";
import { RepoRow } from "./RepoRow";
import { sortRows } from "./sortRows";

interface RepoListProps {
  rows: Row[];
  details: Record<string, string>;
  isBusy: (path: string) => boolean;
  /** A needs-you repo the owner dismissed: listed as a row, not a callout. */
  isDismissed: (path: string) => boolean;
  paused: boolean;
  now: Date;
  onSync: (path: string) => void;
  onOpen: (path: string) => void;
  onSettings: (path: string) => void;
  onDismiss: (path: string) => void;
  /** The body when nothing is listed and there is no `found` section. */
  empty: ReactNode;
  /** The Found section, under the rows in the same scroll area (and alone when nothing is listed). */
  found?: ReactNode;
}

/**
 * The scrolling middle of the popover: the stack of callouts for repos that need you, then one
 * line per repo, dismissed needs-you repos first, then the Found section.
 */
export function RepoList({
  rows,
  details,
  isBusy,
  isDismissed,
  paused,
  now,
  onSync,
  onOpen,
  onSettings,
  onDismiss,
  empty,
  found,
}: RepoListProps) {
  if (rows.length === 0 && !found) return <>{empty}</>;
  const sorted = sortRows(rows);
  const isCallout = (row: Row) => row.health === "needs-you" && !isDismissed(row.path);
  const callouts = sorted.filter(isCallout);
  const others = sorted.filter((row) => !isCallout(row));
  return (
    <ScrollArea.Root className="relative min-h-0 flex-1">
      <ScrollArea.Viewport className="h-full focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus">
        {callouts.length > 0 && (
          <div className="px-3 pt-3 pb-1">
            <CalloutStack
              rows={callouts}
              details={details}
              isBusy={isBusy}
              now={now}
              onSync={onSync}
              onOpen={onOpen}
              onSettings={onSettings}
              onDismiss={onDismiss}
            />
          </div>
        )}
        {others.length > 0 && (
          <ul
            aria-label="Repositories"
            className={`flex flex-col px-2 ${callouts.length > 0 ? "py-1" : "pt-2 pb-1"}`}
          >
            {others.map((row) => (
              <RepoRow
                key={row.path}
                row={row}
                detail={details[row.path]}
                busy={isBusy(row.path)}
                paused={paused}
                now={now}
                onSync={onSync}
                onOpen={onOpen}
                onSettings={onSettings}
              />
            ))}
          </ul>
        )}
        {found}
      </ScrollArea.Viewport>
      <ScrollArea.Scrollbar className="flex w-2 justify-center py-1 opacity-0 data-hovering:opacity-100 data-scrolling:opacity-100">
        <ScrollArea.Thumb className="w-full rounded-full bg-border" />
      </ScrollArea.Scrollbar>
    </ScrollArea.Root>
  );
}
