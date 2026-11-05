import { useEffect, useRef } from "react";

/**
 * Calls `onShown` each time the popover is shown again: the window takes the focus (the app
 * focuses it as it shows it) or the page becomes visible. The app emits no event of its own for
 * this, and the webview lives on while the window is hidden, so these are the signals there are.
 */
export function usePopoverShown(onShown: () => void): void {
  const latest = useRef(onShown);
  useEffect(() => {
    latest.current = onShown;
  });
  useEffect(() => {
    const onVisibility = () => {
      if (document.visibilityState === "visible") latest.current();
    };
    const onFocus = () => latest.current();
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, []);
}
