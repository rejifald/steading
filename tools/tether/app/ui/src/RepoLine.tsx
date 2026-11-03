import type { ReactNode } from "react";

import type { Health, Mode } from "./api";
import { DashedCircleIcon } from "@steading/tray-popover/controls/icons";
import { GLYPHS, GLYPH_CLASSES, StateLabel, glyphOf } from "./StateLabel";

interface RepoLineProps {
  name: string;
  branch: string;
  mode: Mode;
  /** Omitted only for a preview, which has no state yet. */
  health?: Health;
  /** Commits to pull, shown after `behind`. */
  behind?: number | undefined;
  /** A sync of this repo is running: the spinning arc and `syncing…`. */
  busy?: boolean;
  /** tether is paused: the pause glyph; the word stays. */
  paused?: boolean;
  /** The add form's preview: a dashed circle and `not listed yet`. */
  preview?: boolean;
  /** Draws the line as a placeholder hidden from assistive technology. */
  ghost?: boolean;
  /** The full path, on hover of the truncated name and branch. */
  title?: string;
  /** Sits between the branch and the state column (the row's hover actions). */
  children?: ReactNode;
}

/** One display-only line for a repository: state glyph, name, branch, optional actions, state. */
export function RepoLine({
  name,
  branch,
  mode,
  health,
  behind,
  busy = false,
  paused = false,
  preview = false,
  ghost = false,
  title,
  children,
}: RepoLineProps) {
  return (
    <div aria-hidden={ghost || preview || undefined} className="flex h-8 items-center gap-2 px-2">
      <LeadingGlyph health={health} busy={busy} paused={paused} />
      <span className="truncate font-medium" title={title}>
        {name}
      </span>
      <span className="flex-1 truncate text-faint" title={title}>
        {mode === "mirror" ? `${branch} · mirror` : branch}
      </span>
      {children}
      <span className="w-22 shrink-0 text-xs">
        {health && !preview ? (
          <StateLabel health={health} behind={behind} busy={busy} />
        ) : (
          <span className="text-faint">not listed yet</span>
        )}
      </span>
    </div>
  );
}

function LeadingGlyph({ health, busy, paused }: { health?: Health | undefined; busy: boolean; paused: boolean }) {
  if (!health) return <DashedCircleIcon className="shrink-0 text-faint" />;
  const glyph = glyphOf(health, busy, paused);
  const Icon = GLYPHS[glyph];
  return <Icon className={`shrink-0 ${GLYPH_CLASSES[glyph]}`} />;
}
