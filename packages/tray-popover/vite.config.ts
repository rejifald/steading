import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

/** Tests only: the package has no build of its own; each app bundles the sources it imports. */
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
  },
});
