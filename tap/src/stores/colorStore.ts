import { create } from "zustand";

import { api } from "../lib/ipc";
import type { ColorResponse } from "../lib/types";
import { useCursorStore } from "./cursorStore";
import { log } from "./logStore";

/** The color read under the cursor by the Color Picker card. */
interface ColorStore {
  picked: ColorResponse | null;
  pickAtCursor: () => Promise<void>;
}

export const useColorStore = create<ColorStore>((set) => ({
  picked: null,

  pickAtCursor: async () => {
    const pos = useCursorStore.getState().pos;
    if (!pos) return;
    try {
      const color = await api.getPixelColor(pos.x, pos.y);
      if (color) {
        set({ picked: color });
        log(`Color at (${pos.x}, ${pos.y}): ${color.hex}`);
      }
    } catch (err) {
      log(`Failed to read color: ${String(err)}`);
    }
  },
}));
