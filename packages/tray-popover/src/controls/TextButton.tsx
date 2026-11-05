import { Button } from "@base-ui/react/button";
import type { ComponentProps } from "react";

const VARIANTS = {
  primary: "bg-accent text-accent-text hover:brightness-110",
  quiet: "border border-border bg-surface-raised text-text hover:bg-surface-hover",
  inset: "border border-border bg-surface text-text hover:bg-surface-hover",
  /** An action inside a sentence: underlined text, no box, no hover background. Compact size only. */
  link: "bg-transparent text-text underline",
} as const;

/** What differs in type size and padding by variant; the sizes below only set height and weight. */
const SPACING = {
  primary: "px-3 text-sm",
  quiet: "px-3 text-sm",
  inset: "px-3 text-sm",
  link: "px-2 text-xs",
} as const;

/** `compact` sits inside a row or a section heading. */
const SIZES = {
  default: "h-7 font-medium",
  compact: "h-6 font-normal",
} as const;

type TextButtonProps = ComponentProps<typeof Button> & {
  variant?: keyof typeof VARIANTS;
  size?: keyof typeof SIZES;
};

/** A labelled button in one of the styleguide's weights; `link` goes with `size="compact"` only. */
export function TextButton({ variant = "quiet", size = "default", className = "", ...props }: TextButtonProps) {
  return (
    <Button
      {...props}
      className={`inline-flex items-center justify-center rounded-control ${SPACING[variant]} whitespace-nowrap focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-focus disabled:opacity-50 ${SIZES[size]} ${VARIANTS[variant]} ${className}`}
    />
  );
}
