// Shared helpers for store tests.

import { vi } from "vitest";

import type { DslTimedAction, MacroDocument, ValidationErrorResponse } from "../lib/types";
import type { MockApi } from "./ipcMock";

/**
 * Load fresh copies of the stores (and the mocked `api`).
 *
 * `documentStore` keeps its debounce/in-flight bookkeeping in module scope, so every test needs a
 * new module registry or state would leak between tests. The calling test file must have
 * registered the `../lib/ipc` mock via `vi.mock`.
 */
export async function freshStores() {
  vi.resetModules();
  const { api } = await import("../lib/ipc");
  const { useDocumentStore } = await import("../stores/documentStore");
  const { useEngineStore } = await import("../stores/engineStore");
  const { useLogStore } = await import("../stores/logStore");
  return { api: api as unknown as MockApi, useDocumentStore, useEngineStore, useLogStore };
}

export interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason: unknown) => void;
}

export function deferred<T = void>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** A click step at `x` (distinguishable by `x`), `atMs` into the timeline. */
export function click(x: number, atMs = 0): DslTimedAction {
  return { at_ms: atMs, action: { click: { x, y: 0, button: "left" } } };
}

export function wait(ms: number, atMs = 0): DslTimedAction {
  return { at_ms: atMs, action: { wait: { ms } } };
}

export function macro(name: string, timeline: DslTimedAction[] = [click(1)]): MacroDocument {
  return {
    name,
    version: "1.0",
    timeline,
    run: { repeat: 0, start_delay_ms: 3000, speed: 1 },
  };
}

/** Prime the backend calls `documentStore` makes. */
export function primeBackend(
  api: MockApi,
  opts: { doc?: MacroDocument; issues?: ValidationErrorResponse[] } = {},
): void {
  api.getDocument.mockResolvedValue(opts.doc ?? macro("Backend"));
  api.updateDocument.mockResolvedValue(opts.issues ?? []);
}
