import { Collapsible } from "@base-ui/react/collapsible";
import { type ReactNode, useId } from "react";

import { ChevronRightIcon, CrossIcon, DashedCircleIcon, SpinnerIcon, WarningIcon } from "./controls/icons";
import { IconButton } from "./controls/IconButton";
import { TextButton } from "./controls/TextButton";

const FOCUS_RING = "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus";

/** Widths of the placeholder bars, cycled; `w-30` is 120px. */
const GHOST_WIDTHS = ["w-24", "w-16", "w-30"] as const;

interface FoundRowProps {
  /** The clone's folder name. */
  name: string;
  /** What the tool knows about it ("main · ~/Development", "7 worktrees · ~/Development"). */
  detail: string;
  /** The full path: the tooltip on the name and the detail. */
  title: string;
  onAdd: () => void;
  addLabel: string;
  onSkip: () => void;
  /** The ✕'s tooltip and accessible name ("Skip <name>"). */
  skipLabel: string;
  /** An *Add* is in flight: *Add* is disabled and the row stays. */
  busy?: boolean | undefined;
}

/** One suggestion: a dashed glyph, its name and detail, *Add* and ✕. Render inside `FoundSection`. */
export function FoundRow({ name, detail, title, onAdd, addLabel, onSkip, skipLabel, busy = false }: FoundRowProps) {
  const nameId = useId();
  return (
    <li className="flex h-8 items-center gap-2 rounded-control px-2">
      <DashedCircleIcon className="shrink-0 text-faint" />
      <span id={nameId} title={title} className="truncate font-medium">
        {name}
      </span>
      <span title={title} className="flex-1 truncate text-faint">
        {detail}
      </span>
      <TextButton
        size="compact"
        variant="inset"
        onClick={onAdd}
        disabled={busy}
        aria-busy={busy}
        aria-describedby={nameId}
      >
        {addLabel}
      </TextButton>
      <IconButton label={skipLabel} onClick={onSkip}>
        <CrossIcon />
      </IconButton>
    </li>
  );
}

interface FoundPreviewProps {
  /** Two or so made-up clones; the box is decoration. */
  rows: { name: string; detail: string }[];
  /** The *Add* pill's text. It is drawn, not a button. */
  addLabel: string;
}

/** The empty state's dashed box of example rows: faded, hidden from assistive tech, nothing to press. */
export function FoundPreview({ rows, addLabel }: FoundPreviewProps) {
  return (
    <div aria-hidden="true" className="box-border w-full rounded-control border border-dashed border-border py-1 opacity-50">
      {rows.map((row) => (
        <div key={row.name} className="flex h-8 items-center gap-2 px-2">
          <DashedCircleIcon className="shrink-0 text-faint" />
          <span className="truncate font-medium">{row.name}</span>
          <span className="flex-1 truncate text-faint">{row.detail}</span>
          <span className="inline-flex h-6 shrink-0 items-center rounded-control border border-border px-3">
            {addLabel}
          </span>
        </div>
      ))}
    </div>
  );
}

interface FoundSectionProps {
  /** `open` is the heading and its rows; `collapsed` one line that expands; `settled` the line with nothing left. */
  mode: "open" | "collapsed" | "settled";
  /** "Found". */
  title: string;
  /** Suggestions left; a plain number beside the title. */
  count: number;
  /** One sentence under the heading (open only). */
  lead?: string | undefined;
  /** The heading's right slot when nothing is scanning: an action such as *Where to look…*. */
  aside?: ReactNode | undefined;
  /** While a scan runs: the status at the heading's right ("looking in ~/Code…"), spinner first. Replaces `aside`. */
  scanning?: string | undefined;
  /** One warning line per root that could not be read. */
  errors?: string[] | undefined;
  /** The `FoundRow`s. */
  children?: ReactNode | undefined;
  /** Placeholder rows after the real ones, while a scan runs (open only). */
  ghostRows?: number | undefined;
  /** Collapsed: the right of the title ("· looked 3 days ago"). */
  collapsedSummary?: string | undefined;
  /** Settled: the line's text ("No suggestions · looked just now"). */
  settledText?: string | undefined;
  /** Collapsed and settled: *Look again*. The button shows only with both this and its label. */
  onLookAgain?: (() => void) | undefined;
  lookAgainLabel?: string | undefined;
  /** Listed rows sit above: draw the rule that separates them (open only; the other modes always have it). */
  hasRuleAbove?: boolean | undefined;
}

function ErrorLines({ errors }: { errors: string[] | undefined }) {
  if (!errors || errors.length === 0) return null;
  return (
    <div>
      {errors.map((error) => (
        <p key={error} className="flex gap-2 px-4 pb-1 text-xs text-text">
          <WarningIcon className="shrink-0 text-needs-you" />
          {error}
        </p>
      ))}
    </div>
  );
}

function LookAgain({ onLookAgain, lookAgainLabel }: Pick<FoundSectionProps, "onLookAgain" | "lookAgainLabel">) {
  if (!onLookAgain || !lookAgainLabel) return null;
  return (
    <TextButton size="compact" variant="quiet" onClick={onLookAgain}>
      {lookAgainLabel}
    </TextButton>
  );
}

/**
 * The clones a scan found, in either tool: an open heading with rows, one collapsed line, or the
 * settled line once none is left. The caller supplies every word.
 */
export function FoundSection({
  mode,
  title,
  count,
  lead,
  aside,
  scanning,
  errors,
  children,
  ghostRows = 0,
  collapsedSummary,
  settledText,
  onLookAgain,
  lookAgainLabel,
  hasRuleAbove = false,
}: FoundSectionProps) {
  const headingId = useId();

  if (mode === "settled") {
    return (
      <section aria-label={title}>
        <div className="flex min-h-8 items-center gap-2 border-t border-border px-4 py-1.5">
          <DashedCircleIcon className="shrink-0 text-faint" />
          <span className="flex-1 text-xs text-faint">{settledText}</span>
          <LookAgain onLookAgain={onLookAgain} lookAgainLabel={lookAgainLabel} />
        </div>
      </section>
    );
  }

  if (mode === "collapsed") {
    return (
      <section aria-label={title}>
        <Collapsible.Root>
          {/* Look again is the toggle's sibling: a button never nests in a button. */}
          <div className="flex min-h-8 items-center gap-2 border-t border-border px-4 py-1.5">
            <Collapsible.Trigger
              className={`group flex flex-1 items-center gap-2 rounded-control text-left text-text ${FOCUS_RING}`}
            >
              <ChevronRightIcon className="shrink-0 text-muted group-data-panel-open:rotate-90" />
              <span className="font-semibold">{title}</span>
              <span className="text-faint">{count}</span>
              {collapsedSummary && <span className="text-xs text-faint">{collapsedSummary}</span>}
            </Collapsible.Trigger>
            <LookAgain onLookAgain={onLookAgain} lookAgainLabel={lookAgainLabel} />
          </div>
          <Collapsible.Panel>
            <ErrorLines errors={errors} />
            <ul aria-label={title} className="px-2 pb-1">
              {children}
            </ul>
          </Collapsible.Panel>
        </Collapsible.Root>
      </section>
    );
  }

  return (
    <section aria-labelledby={headingId}>
      {/* The rule is always laid out (transparent without listed rows), so the heading never shifts. */}
      <div
        className={`flex items-center gap-2 border-t px-4 pt-3 pb-1 ${hasRuleAbove ? "border-border" : "border-transparent"}`}
      >
        <h2 className="flex gap-2 text-sm font-semibold text-text">
          <span id={headingId}>{title}</span> <span className="font-normal text-faint">{count}</span>
        </h2>
        {/* Always in the tree, so a screen reader hears each text change; it keeps the right edge. */}
        <span role="status" aria-live="polite" className="ml-auto flex items-center gap-1 text-xs text-progress">
          {scanning && (
            <>
              <SpinnerIcon className="shrink-0 animate-spin motion-reduce:animate-none" />
              {scanning}
            </>
          )}
        </span>
        {!scanning && aside}
      </div>
      {lead && <p className="px-4 pb-1 text-xs text-muted">{lead}</p>}
      <ErrorLines errors={errors} />
      <ul aria-labelledby={headingId} className="px-2 pb-1">
        {children}
      </ul>
      {ghostRows > 0 && (
        <ul aria-hidden="true" className="px-2 opacity-50">
          {Array.from({ length: ghostRows }, (_, index) => (
            <li key={index} className="flex h-8 items-center gap-2 px-2">
              <DashedCircleIcon className="shrink-0 text-faint" />
              <span className={`h-2.5 rounded-full bg-border ${GHOST_WIDTHS[index % GHOST_WIDTHS.length]}`} />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
