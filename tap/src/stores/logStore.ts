import { create } from "zustand";

import type { LogEntry } from "../lib/types";

const LOG_CAP = 200;

/** The activity log. A leaf store: every other store may write to it, it depends on none. */
export const useLogStore = create<{ logs: LogEntry[] }>(() => ({ logs: [] }));

export function log(message: string): void {
  useLogStore.setState((s) => ({ logs: [...s.logs, { time: nowTime(), message }].slice(-LOG_CAP) }));
}

export function clearLog(): void {
  useLogStore.setState({ logs: [] });
}

function nowTime(): string {
  const now = new Date();
  const pad = (n: number, width = 2) => n.toString().padStart(width, "0");
  return `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}.${pad(now.getMilliseconds(), 3)}`;
}
