import { NeedsYouIcon } from "./controls/icons";

interface NoticeStripProps {
  /** A click that failed here, until the next one. */
  actionError: string | null;
  /** The app's last failed action (Pause, a sync), until the next refresh. */
  note: string | null;
  /** repos.toml could not be read but an earlier report is still shown. */
  staleError: string | null;
}

/** What went wrong, verbatim, in a tinted strip under the header; nothing when all is well. */
export function NoticeStrip({ actionError, note, staleError }: NoticeStripProps) {
  const text =
    actionError ?? note ?? (staleError ? `Cannot read repos.toml: ${staleError}. Showing the last report.` : null);
  if (!text) return null;
  return (
    <p role="alert" className="flex items-start gap-2 border-b border-border bg-needs-you/12 px-4 py-2 text-xs text-text">
      <NeedsYouIcon className="shrink-0 text-needs-you" />
      {text}
    </p>
  );
}
