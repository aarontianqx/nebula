// 工具注册表状态：从后端拉取工具列表，按分类分组。

import { create } from "zustand";
import { api } from "../lib/ipc";
import type { Category, ToolDescriptor } from "../types/tool";

const FAV_KEY = "pulsar.favorites";

function loadFavorites(): string[] {
  try {
    const raw = localStorage.getItem(FAV_KEY);
    return raw ? (JSON.parse(raw) as string[]) : [];
  } catch {
    return [];
  }
}

interface RegistryState {
  tools: ToolDescriptor[];
  loading: boolean;
  error: string | null;
  query: string;
  favorites: string[];
  fetchTools: () => Promise<void>;
  setQuery: (q: string) => void;
  toggleFavorite: (id: string) => void;
  isFavorite: (id: string) => boolean;
  filtered: () => ToolDescriptor[];
  favoriteTools: () => ToolDescriptor[];
  byCategory: () => Map<Category, ToolDescriptor[]>;
}

export const useRegistryStore = create<RegistryState>((set, get) => ({
  tools: [],
  loading: false,
  error: null,
  query: "",
  favorites: loadFavorites(),

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

  toggleFavorite: (id) =>
    set((state) => {
      const favorites = state.favorites.includes(id)
        ? state.favorites.filter((f) => f !== id)
        : [...state.favorites, id];
      try {
        localStorage.setItem(FAV_KEY, JSON.stringify(favorites));
      } catch {
        // 忽略存储失败（隐私模式等）
      }
      return { favorites };
    }),

  isFavorite: (id) => get().favorites.includes(id),

  favoriteTools: () => {
    const { tools, favorites } = get();
    return favorites
      .map((id) => tools.find((t) => t.id === id))
      .filter((t): t is ToolDescriptor => Boolean(t));
  },

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
