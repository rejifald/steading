/** One entry of the *Pull every* menu: what `repos.toml` stores and what the menu says. */
export interface IntervalChoice {
  /** As `repos.toml` and `git tether set --interval` spell it: `<n>s`, `<n>m` or `<n>h`. */
  value: string;
  seconds: number;
  label: string;
}

const MINUTE = 60;
const HOUR = 3600;

export const PRESETS: readonly IntervalChoice[] = [
  { value: "1m", seconds: MINUTE, label: "1 min" },
  { value: "5m", seconds: 5 * MINUTE, label: "5 min" },
  { value: "15m", seconds: 15 * MINUTE, label: "15 min" },
  { value: "1h", seconds: HOUR, label: "1 hour" },
];

/** tether-core's `DEFAULT_INTERVAL`. */
export const DEFAULT_INTERVAL_SECONDS = 5 * MINUTE;

/** The choice for `seconds`: a preset, or one spelled from the value itself. */
export function choiceOf(seconds: number): IntervalChoice {
  const preset = PRESETS.find((choice) => choice.seconds === seconds);
  if (preset) return preset;
  if (seconds % HOUR === 0) return custom(`${seconds / HOUR}h`, seconds, `${seconds / HOUR} h`);
  if (seconds % MINUTE === 0) {
    return custom(`${seconds / MINUTE}m`, seconds, `${seconds / MINUTE} min`);
  }
  return custom(`${seconds}s`, seconds, `${seconds} s`);
}

/** The presets, plus a repo's own interval when `repos.toml` sets one they lack, in order. */
export function choicesFor(seconds: number): IntervalChoice[] {
  const current = choiceOf(seconds);
  if (PRESETS.includes(current)) return [...PRESETS];
  return [...PRESETS, current].sort((a, b) => a.seconds - b.seconds);
}

function custom(value: string, seconds: number, label: string): IntervalChoice {
  return { value, seconds, label: `${label} (custom)` };
}
