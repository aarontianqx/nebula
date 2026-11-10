import { create } from "zustand";

import type { Point } from "../lib/types";

/** Where the pointer is on the screen, as last reported by the backend. */
export const useCursorStore = create<{ pos: Point | null }>(() => ({ pos: null }));

export function setCursorPos(pos: Point): void {
  useCursorStore.setState({ pos });
}
