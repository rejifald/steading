import { TextButton } from "./controls/TextButton";

interface ReadErrorProps {
  message: string;
  onRetry: () => void;
}

/** The full-panel body when repos.toml cannot be read and there is no earlier report to show. */
export function ReadError({ message, onRetry }: ReadErrorProps) {
  return (
    <div role="alert" className="flex flex-1 flex-col items-center justify-center gap-2 p-6 text-center">
      <span aria-hidden="true" className="size-3 rounded-full bg-needs-you" />
      <p className="text-base font-semibold text-text">Cannot read repos.toml</p>
      <p className="text-xs text-muted">{message}</p>
      <p className="text-sm text-muted">Fix the file, then try again. Nothing is checked until it reads.</p>
      <TextButton variant="primary" className="mt-2" onClick={onRetry}>
        Try again
      </TextButton>
    </div>
  );
}
