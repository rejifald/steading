import { WarningIcon } from "./controls/icons";

interface NoticeStripProps {
  /** What went wrong, verbatim; nothing renders without it. */
  text: string | null;
}

/** A needs-you-tinted strip under the header for a failure the popover can still work around. */
export function NoticeStrip({ text }: NoticeStripProps) {
  if (!text) return null;
  return (
    <p role="alert" className="flex items-start gap-2 border-b border-border bg-needs-you/12 px-4 py-2 text-xs text-text">
      <WarningIcon className="shrink-0 text-needs-you" />
      {text}
    </p>
  );
}
