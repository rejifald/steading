import { CSPProvider } from "@base-ui/react/csp-provider";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { tauriApi, type TetherApi } from "./api";
import { Popover } from "./Popover";
import "./theme.css";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

/** Inside the app, the real commands; in a plain browser (`npm run dev`), the fake. */
async function chooseApi(): Promise<TetherApi> {
  if (window.__TAURI_INTERNALS__) return tauriApi;
  const { createFakeApi } = await import("./api.fake");
  const scenario = new URLSearchParams(window.location.search).get("scenario");
  if (scenario === null) return createFakeApi();
  const { SCENARIOS } = await import("./api.scenarios");
  return createFakeApi(SCENARIOS[scenario] ?? {});
}

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root element");

void chooseApi().then((api) =>
  createRoot(root).render(
    <StrictMode>
      {/* theme.css carries Base UI's scrollbar rule, so the CSP needs no inline styles. */}
      <CSPProvider disableStyleElements>
        <Popover api={api} />
      </CSPProvider>
    </StrictMode>,
  ),
);
