import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { SessionInfo, SessionState } from "../types";

export type { SessionInfo, SessionState };

interface SessionStore {
  sessions: SessionInfo[];
  frames: Record<string, string>; // session_id -> base64 frame
  selectedSessionId: string | null;
  loading: boolean;
  error: string | null;

  // Actions
  fetchSessions: () => Promise<void>;
  startSession: (accountId: string) => Promise<string>;
  stopSession: (sessionId: string) => Promise<void>;
  stopAllSessions: () => Promise<void>;
  clickSession: (sessionId: string, x: number, y: number) => Promise<void>;
  clickAllSessions: (x: number, y: number) => Promise<void>;
  dragSession: (
    sessionId: string,
    fromX: number,
    fromY: number,
    toX: number,
    toY: number
  ) => Promise<void>;
  selectSession: (sessionId: string | null) => void;

  // Event handlers (called from useTauriEvents)
  addSession: (session: SessionInfo) => void;
  updateSessionState: (sessionId: string, state: SessionState) => void;
  removeSession: (sessionId: string) => void;
  setFrame: (sessionId: string, frame: string) => void;
}

export const useSessionStore = create<SessionStore>((set) => ({
  sessions: [],
  frames: {},
  selectedSessionId: null,
  loading: false,
  error: null,

  fetchSessions: async () => {
    set({ loading: true, error: null });
    try {
      const sessions = await invoke<SessionInfo[]>("get_sessions");
      set({ sessions, loading: false });
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },

  startSession: async (accountId: string) => {
    set({ loading: true, error: null });
    try {
      const sessionId = await invoke<string>("start_session", {
        accountId,
      });
      set({ loading: false, selectedSessionId: sessionId });
      return sessionId;
    } catch (error) {
      set({ error: String(error), loading: false });
      throw error;
    }
  },

  stopSession: async (sessionId: string) => {
    try {
      await invoke("stop_session", { sessionId });
    } catch (error) {
      set({ error: String(error) });
    }
  },

  stopAllSessions: async () => {
    try {
      await invoke("stop_all_sessions");
    } catch (error) {
      set({ error: String(error) });
    }
  },

  clickSession: async (sessionId: string, x: number, y: number) => {
    try {
      await invoke("click_session", { sessionId, x, y });
    } catch (error) {
      console.error("Click failed:", error);
    }
  },

  clickAllSessions: async (x: number, y: number) => {
    try {
      await invoke("click_all_sessions", { x, y });
    } catch (error) {
      console.error("Click all failed:", error);
    }
  },

  dragSession: async (
    sessionId: string,
    fromX: number,
    fromY: number,
    toX: number,
    toY: number
  ) => {
    try {
      await invoke("drag_session", { sessionId, fromX, fromY, toX, toY });
    } catch (error) {
      console.error("Drag failed:", error);
    }
  },

  selectSession: (sessionId: string | null) => {
    set({ selectedSessionId: sessionId });
  },

  // Event handlers
  addSession: (session: SessionInfo) => {
    set((state) => {
      // Prevent duplicate sessions (can happen with React StrictMode or race conditions)
      if (state.sessions.some((s) => s.id === session.id)) {
        return state;
      }
      return { sessions: [...state.sessions, session] };
    });
  },

  updateSessionState: (sessionId: string, newState: SessionState) => {
    set((state) => ({
      sessions: state.sessions.map((s) =>
        s.id === sessionId ? { ...s, state: newState } : s
      ),
    }));
  },

  removeSession: (sessionId: string) => {
    set((state) => ({
      sessions: state.sessions.filter((s) => s.id !== sessionId),
      frames: Object.fromEntries(
        Object.entries(state.frames).filter(([id]) => id !== sessionId)
      ),
      selectedSessionId:
        state.selectedSessionId === sessionId ? null : state.selectedSessionId,
    }));
  },

  setFrame: (sessionId: string, frame: string) => {
    set((state) => ({
      frames: { ...state.frames, [sessionId]: frame },
    }));
  },
}));

