import { Button } from "@base-ui/react/button";
import type { ComponentProps } from "react";

const VARIANTS = {
  primary: "bg-accent text-accent-text hover:brightness-110",
  quiet: "border border-border bg-surface-raised text-text hover:bg-surface-hover",
  inset: "border border-border bg-surface text-text hover:bg-surface-hover",
} as const;

type TextButtonProps = ComponentProps<typeof Button> & { variant?: keyof typeof VARIANTS };

/** A labelled button in one of the styleguide's weights. */
export function TextButton({ variant = "quiet", className = "", ...props }: TextButtonProps) {
  return (
    <Button
      {...props}
      className={`inline-flex h-7 items-center justify-center rounded-control px-3 text-sm font-medium whitespace-nowrap focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-focus disabled:opacity-50 ${VARIANTS[variant]} ${className}`}
    />
  );
}
