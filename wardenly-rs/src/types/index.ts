// Domain types shared between frontend and backend

export interface Account {
  id: string;
  role_name: string;
  user_name: string;
  password: string;
  server_id: number;
  ranking: number;
  cookies?: string;
}

export interface Group {
  id: string;
  name: string;
  description?: string;
  account_ids: string[];
  ranking: number;
}

// Using string literal type for better TypeScript interop with Tauri
export type SessionState =
  | "Idle"
  | "Starting"
  | "LoggingIn"
  | "Ready"
  | "ScriptRunning"
  | "Stopped";

// Enum version for use in UI logic
export const SessionStateEnum = {
  Idle: "Idle" as const,
  Starting: "Starting" as const,
  LoggingIn: "LoggingIn" as const,
  Ready: "Ready" as const,
  ScriptRunning: "ScriptRunning" as const,
  Stopped: "Stopped" as const,
};

export interface SessionInfo {
  id: string;
  account_id: string;
  display_name: string;
  state: SessionState;
}

export interface ScriptInfo {
  name: string;
  description?: string;
}

