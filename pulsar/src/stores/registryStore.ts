// 工具注册表状态：从后端拉取工具列表，按分类分组。

import { create } from "zustand";
import { api } from "../lib/ipc";
import type { Category, ToolDescriptor } from "../types/tool";

interface RegistryState {
  tools: ToolDescriptor[];
  loading: boolean;
  error: string | null;
  query: string;
  fetchTools: () => Promise<void>;
  setQuery: (q: string) => void;
  filtered: () => ToolDescriptor[];
  byCategory: () => Map<Category, ToolDescriptor[]>;
}

export const useRegistryStore = create<RegistryState>((set, get) => ({
  tools: [],
  loading: false,
  error: null,
  query: "",

  fetchTools: async () => {
    set({ loading: true, error: null });
    try {
      const tools = await api.listTools();
      set({ tools, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  setQuery: (q) => set({ query: q }),

  filtered: () => {
    const { tools, query } = get();
    const q = query.trim().toLowerCase();
    if (!q) return tools;
    return tools.filter(
      (t) =>
        t.name.toLowerCase().includes(q) ||
        t.id.includes(q) ||
        t.keywords.some((k) => k.toLowerCase().includes(q))
    );
  },

  byCategory: () => {
    const map = new Map<Category, ToolDescriptor[]>();
    for (const t of get().filtered()) {
      const list = map.get(t.category) ?? [];
      list.push(t);
      map.set(t.category, list);
    }
    return map;
  },
}));
