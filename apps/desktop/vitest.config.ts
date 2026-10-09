import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    // Release tooling tests live next to their scripts (see docs/releasing.md).
    include: ["src/**/*.test.{ts,tsx}", "../../scripts/**/*.test.mjs"],
    setupFiles: ["./src/test/setup.ts"],
  },
});
