import type { AriaRole, ReactNode } from "react";

const STRIP_TONES = {
  "needs-you": "bg-needs-you/14 border-needs-you/30",
  plain: "bg-surface-raised border-border",
} as const;

interface CalloutProps {
  /** `needs-you` tints the top strip; `plain` is for what is not a state, as a confirm. */
  tone: keyof typeof STRIP_TONES;
  /** The top strip's content. */
  heading: ReactNode;
  children?: ReactNode;
  role?: AriaRole;
  "aria-label"?: string;
}

/** A raised box with a top strip and a body: needs-you repos, errors, a confirm. */
export function Callout({ tone, heading, children, role, "aria-label": ariaLabel }: CalloutProps) {
  return (
    <section
      role={role}
      aria-label={ariaLabel}
      className="shrink-0 overflow-hidden rounded-control border border-border bg-surface-raised shadow-raised"
    >
      <div className={`flex items-center gap-2 border-b px-3 py-2 text-sm text-text ${STRIP_TONES[tone]}`}>
        {heading}
      </div>
      {children !== undefined && <div className="flex flex-col gap-2 p-3 text-sm">{children}</div>}
    </section>
  );
}
