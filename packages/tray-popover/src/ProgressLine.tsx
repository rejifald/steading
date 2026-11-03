interface ProgressLineProps {
  /** Inset to a row's padding; otherwise full width on the header's bottom border. */
  inset?: boolean;
}

/** A 2px hairline with a bar sweeping along it while something runs; still under reduced motion. */
export function ProgressLine({ inset = false }: ProgressLineProps) {
  const place = inset ? "inset-x-2 bottom-0" : "inset-x-0 -bottom-px";
  return (
    <span
      aria-hidden="true"
      className={`pointer-events-none absolute h-0.5 overflow-hidden rounded-full bg-progress/25 ${place}`}
    >
      <span className="block h-full w-2/5 animate-progress rounded-full bg-progress motion-reduce:animate-none" />
    </span>
  );
}
