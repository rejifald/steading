import type { ReactNode } from "react";

import { Callout } from "@steading/tray-popover/Callout";
import { WarningIcon } from "@steading/tray-popover/controls/icons";

interface NeedsYouCalloutProps {
  /** The branch, or the repo's name for a repo error; truncates. */
  title: string;
  /** Faint on the right of the strip: `repo · age`. */
  meta?: string | undefined;
  /** What went wrong, in full; it wraps. */
  sentence: string;
  /** The text buttons under the sentence. */
  children: ReactNode;
}

/** A raised needs-you box: triangle, title and meta in the tinted strip, the sentence and actions below. */
export function NeedsYouCallout({ title, meta, sentence, children }: NeedsYouCalloutProps) {
  return (
    <Callout
      tone="needs-you"
      heading={
        <>
          <WarningIcon className="shrink-0 text-needs-you" />
          <span className="min-w-0 flex-1 truncate font-medium">{title}</span>
          {meta && <span className="shrink-0 text-xs text-muted">{meta}</span>}
        </>
      }
    >
      <p className="text-xs text-needs-you">{sentence}</p>
      <div className="flex gap-2">{children}</div>
    </Callout>
  );
}
