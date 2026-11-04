import { WarningIcon } from "./controls/icons";
import { TextButton } from "./controls/TextButton";

interface ReadErrorProps {
  /** What could not be read, as `Cannot read repos.toml`. */
  title: string;
  /** The error, verbatim. */
  message: string;
  /** What to do, and what stops until then. */
  hint: string;
  onRetry: () => void;
}

/** The full-panel body when the tool's config cannot be read and there is no earlier report to show. */
export function ReadError({ title, message, hint, onRetry }: ReadErrorProps) {
  return (
    <div role="alert" className="flex flex-1 flex-col items-center justify-center gap-2 p-6 text-center">
      <WarningIcon className="size-7 text-needs-you" />
      <p className="text-base font-semibold text-text">{title}</p>
      <p className="font-mono text-xs text-muted">{message}</p>
      <p className="text-sm text-muted">{hint}</p>
      <TextButton variant="primary" className="mt-2" onClick={onRetry}>
        Try again
      </TextButton>
    </div>
  );
}
