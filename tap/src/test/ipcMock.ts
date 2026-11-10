// Test double for `src/lib/ipc.ts`.
//
// Usage (the factory must be passed to `vi.mock` from the test file itself so it is hoisted):
//
//   vi.mock("../lib/ipc", async () => (await import("../test/ipcMock")).createIpcMock());
//
// Every `api.<method>` is created lazily as a `vi.fn()` that resolves to `undefined`; tests prime
// the ones they care about with `mockResolvedValue` / `mockImplementation`.

import { vi, type Mock } from "vitest";

export type MockApi = Record<string, Mock>;

export function createIpcMock() {
  const fns: MockApi = {};
  const api = new Proxy(fns, {
    get(target, prop) {
      // Never look like a thenable (`await api`) and ignore symbol probes.
      if (typeof prop !== "string" || prop === "then") return undefined;
      return (target[prop] ??= vi.fn());
    },
  });

  return {
    api,
    isTauri: () => true,
    pickSavePath: vi.fn(async () => null),
    pickOpenPath: vi.fn(async () => null),
    listen: vi.fn(async () => () => {}),
  };
}
