import { useEffect } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { useSessionStore, SessionState } from "../stores/sessionStore";

interface SessionCreatedPayload {
  session_id: string;
  account_id: string;
  display_name: string;
}

interface SessionStateChangedPayload {
  session_id: string;
  old_state: SessionState;
  new_state: SessionState;
}

interface ScreencastFramePayload {
  session_id: string;
  image_base64: string;
  timestamp: number;
}

interface SessionStoppedPayload {
  session_id: string;
}

interface ScriptStoppedPayload {
  session_id: string;
  script_name: string;
}

export function useTauriEvents() {
  const { addSession, updateSessionState, removeSession, setFrame } =
    useSessionStore();

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    // Listen for session created
    listen<SessionCreatedPayload>("session_created", (event) => {
      const payload = event.payload;
      addSession({
        id: payload.session_id,
        account_id: payload.account_id,
        display_name: payload.display_name,
        state: "Idle",
      });
    }).then((u) => unlisteners.push(u));

    // Listen for session state changed
    listen<SessionStateChangedPayload>("session_state_changed", (event) => {
      const payload = event.payload;
      updateSessionState(payload.session_id, payload.new_state);
    }).then((u) => unlisteners.push(u));

    // Listen for screencast frames
    listen<ScreencastFramePayload>("screencast_frame", (event) => {
      const payload = event.payload;
      setFrame(payload.session_id, payload.image_base64);
    }).then((u) => unlisteners.push(u));

    // Listen for session stopped
    listen<SessionStoppedPayload>("session_stopped", (event) => {
      const payload = event.payload;
      removeSession(payload.session_id);
    }).then((u) => unlisteners.push(u));

    // Listen for script stopped
    listen<ScriptStoppedPayload>("script_stopped", (event) => {
      const payload = event.payload;
      console.log(
        `Script ${payload.script_name} stopped on session ${payload.session_id}`
      );
      // When a script stops, the session state changes to Ready
      // This is already handled by session_state_changed event
    }).then((u) => unlisteners.push(u));

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, [addSession, updateSessionState, removeSession, setFrame]);
}

