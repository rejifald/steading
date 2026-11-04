import type { ReactNode } from "react";

/** The popover's bottom band of fixed actions, above the list's scroll. */
export function PopoverFooter({ children }: { children: ReactNode }) {
  return <footer className="flex items-center gap-2 border-t border-border px-4 py-3">{children}</footer>;
}
