import { defineConfig } from "vitest/config";

// Unit tests cover the stores and pure helpers, none of which need a DOM.
// The Tauri bridge (`src/lib/ipc.ts`) is always mocked, see `src/test/ipcMock.ts`.
export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
