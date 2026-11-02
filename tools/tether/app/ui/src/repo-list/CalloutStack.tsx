import { Button } from "@base-ui/react/button";
import { useEffect, useRef, useState } from "react";

import type { Row } from "../api";
import { NeedsYouCallout, type NeedsYouCalloutProps } from "./NeedsYouCallout";

/** At most this many slivers peek under a folded stack, however many callouts it hides. */
const MAX_SLIVERS = 2;

const SLIVER = "absolute h-3 rounded-b-control border border-t-0 border-border bg-surface-raised shadow-raised";
const TOGGLE =
  "ml-auto flex h-6 items-center rounded-control px-2 text-xs font-medium text-text hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-focus";

type CalloutCallbacks = Omit<NeedsYouCalloutProps, "row" | "detail" | "busy">;

interface CalloutStackProps extends CalloutCallbacks {
  /** The needs-you rows still shown as callouts, in list order. */
  rows: Row[];
  details: Record<string, string>;
  isBusy: (path: string) => boolean;
}

/**
 * The needs-you callouts: the first open and the rest folded behind it, until *Show all*.
 * Folds again whenever the popover is hidden (it loses focus), so it reopens folded.
 */
export function CalloutStack({ rows, details, isBusy, ...callbacks }: CalloutStackProps) {
  const [isOpen, setOpen] = useState(false);
  const stack = useRef<HTMLDivElement>(null);
  const showAll = useRef<HTMLButtonElement>(null);
  const focusTarget = useRef<"second" | "toggle" | null>(null);

  useEffect(() => {
    const fold = () => setOpen(false);
    window.addEventListener("blur", fold);
    return () => window.removeEventListener("blur", fold);
  }, []);

  useEffect(() => {
    if (focusTarget.current === "second") {
      stack.current?.querySelectorAll("section")[1]?.querySelector("button")?.focus();
    } else if (focusTarget.current === "toggle") {
      showAll.current?.focus();
    }
    focusTarget.current = null;
  }, [isOpen]);

  const callout = (row: Row) => (
    <NeedsYouCallout
      key={row.path}
      row={row}
      detail={details[row.path]}
      busy={isBusy(row.path)}
      {...callbacks}
    />
  );

  const [first] = rows;
  if (!first) return null;
  if (rows.length === 1) return callout(first);

  if (isOpen) {
    return (
      <div ref={stack} className="flex flex-col gap-2">
        {rows.map(callout)}
        <div className="flex h-6 items-center px-1">
          <Button
            aria-expanded
            className={TOGGLE}
            onClick={() => {
              focusTarget.current = "toggle";
              setOpen(false);
            }}
          >
            Show less
          </Button>
        </div>
      </div>
    );
  }

  const hidden = rows.length - 1;
  return (
    <div className="flex flex-col gap-1">
      <div className="relative pb-3">
        <div className="relative z-2">{callout(first)}</div>
        <span aria-hidden="true" className={`${SLIVER} inset-x-1.5 bottom-1.5 z-1`} />
        {Math.min(hidden, MAX_SLIVERS) === MAX_SLIVERS && (
          <span aria-hidden="true" className={`${SLIVER} inset-x-3 bottom-0`} />
        )}
      </div>
      <div className="flex h-6 items-center px-1">
        <span className="text-xs text-muted">
          {hidden} more {hidden === 1 ? "needs you" : "need you"}
        </span>
        <Button
          ref={showAll}
          aria-expanded={false}
          className={TOGGLE}
          onClick={() => {
            focusTarget.current = "second";
            setOpen(true);
          }}
        >
          Show all
        </Button>
      </div>
    </div>
  );
}
