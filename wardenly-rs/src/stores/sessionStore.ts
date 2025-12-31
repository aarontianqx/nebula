import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { SessionInfo, SessionState } from "../types";

export type { SessionInfo, SessionState };

interface SessionStore {
  sessions: SessionInfo[];
  // Single frame for the currently selected session's display
  // Only updated when receiving frames for the selected session
  currentFrame: string | null;
  selectedSessionId: string | null;
  // Track how the current session was activated
  activationSource: 'account' | 'group' | 'manual' | null;
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
  addSession: (session: SessionInfo, shouldSelect?: boolean) => void;
  updateSessionState: (sessionId: string, state: SessionState) => void;
  removeSession: (sessionId: string) => void;
  setFrame: (sessionId: string, frame: string) => void;
  clearCurrentFrame: () => void;
}

export const useSessionStore = create<SessionStore>((set, get) => ({
  sessions: [],
  currentFrame: null,
  selectedSessionId: null,
  activationSource: null,
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
      set({ loading: false, selectedSessionId: sessionId, activationSource: 'account' });
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
    set({ selectedSessionId: sessionId, activationSource: sessionId ? 'manual' : null });
  },

  // Event handlers
  addSession: (session: SessionInfo, shouldSelect: boolean = false) => {
    set((state) => {
      // Prevent duplicate sessions (can happen with React StrictMode or race conditions)
      if (state.sessions.some((s) => s.id === session.id)) {
        return state;
      }
      const newState: Partial<SessionStore> = { sessions: [...state.sessions, session] };

      // Auto-select if requested or if no session is currently selected
      if (shouldSelect || state.selectedSessionId === null) {
        newState.selectedSessionId = session.id;
        newState.activationSource = shouldSelect ? 'group' : 'manual';
      }

      return newState;
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
    set((state) => {
      const newSessions = state.sessions.filter((s) => s.id !== sessionId);
      let newSelectedId = state.selectedSessionId;
      let newActivationSource = state.activationSource;
      let newCurrentFrame = state.currentFrame;

      // If the removed session was selected, auto-select next session
      if (state.selectedSessionId === sessionId) {
        if (newSessions.length > 0) {
          // Find the index of the removed session
          const oldIndex = state.sessions.findIndex((s) => s.id === sessionId);
          // Select the session at the same index, or the last one if out of bounds
          const nextIndex = Math.min(oldIndex, newSessions.length - 1);
          newSelectedId = newSessions[nextIndex].id;
          newActivationSource = 'manual';
          // Don't clear frame - let the new session's frame overwrite it
        } else {
          newSelectedId = null;
          newActivationSource = null;
          newCurrentFrame = null; // Clear frame only when no sessions left
        }
      }

      return {
        sessions: newSessions,
        selectedSessionId: newSelectedId,
        activationSource: newActivationSource,
        currentFrame: newCurrentFrame,
      };
    });
  },

  // Only update currentFrame if this frame is for the currently selected session
  setFrame: (sessionId: string, frame: string) => {
    const state = get();
    if (state.selectedSessionId === sessionId) {
      set({ currentFrame: frame });
    }
  },

  clearCurrentFrame: () => {
    set({ currentFrame: null });
  },
}));

