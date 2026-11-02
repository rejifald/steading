import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

const base = {
  width: 16,
  height: 16,
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.5,
  strokeLinecap: "round",
  strokeLinejoin: "round",
  "aria-hidden": true,
} as const;

/** Two arrows chasing each other: *Refresh*. */
export function SyncIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <path d="M13 6.5A5 5 0 0 0 4 4.2L3 5.5" />
      <path d="M3 2.5v3h3" />
      <path d="M3 9.5a5 5 0 0 0 9 2.3l1-1.3" />
      <path d="M13 13.5v-3h-3" />
    </svg>
  );
}

/** A folder: *Open in Finder*. */
export function FolderIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <path d="M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.4 1.5h5A1.5 1.5 0 0 1 14 6v5.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z" />
    </svg>
  );
}

/** A pin: *Keep* and *Unkeep*. */
export function PinIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <path d="M6 2.5h4l-.5 4 2.5 2.5H4l2.5-2.5z" />
      <path d="M8 9v4.5" />
    </svg>
  );
}

/** A bin: *Remove now*. */
export function BinIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <path d="M3 4.5h10" />
      <path d="M6.5 4.5V3h3v1.5" />
      <path d="M4.5 4.5l.7 8.5h5.6l.7-8.5" />
    </svg>
  );
}

/** A chevron pointing right: a collapsed section; rotated, an open one. */
export function ChevronIcon(props: IconProps) {
  return (
    <svg {...base} {...props}>
      <path d="M6 3.5L10.5 8 6 12.5" />
    </svg>
  );
}
