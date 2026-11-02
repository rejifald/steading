const NAME_BAR_WIDTHS = ["w-40", "w-33", "w-46", "w-29", "w-37"] as const;

/** Skeleton rows shown until the first report arrives. Still, so nothing moves while it waits. */
export function LoadingRows() {
  return (
    <div aria-busy="true" className="flex flex-1 flex-col gap-5 p-4">
      {NAME_BAR_WIDTHS.map((width) => (
        <div key={width} className="flex items-center gap-2">
          <span className="size-4 shrink-0 rounded-full bg-surface-raised" />
          <span className={`h-3 rounded-control bg-surface-raised ${width}`} />
          <span className="ml-auto h-3 w-14 rounded-control bg-surface-raised" />
        </div>
      ))}
    </div>
  );
}
