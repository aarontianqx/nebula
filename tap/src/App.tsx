import { invoke } from "@tauri-apps/api/core";
import React from "react";

type Profile = {
  name: string;
  timeline: { actions: Array<{ at_ms: number; enabled: boolean; note: string | null; action: unknown }> };
  run: { start_delay_ms: number; speed: number; repeat: unknown };
};

export default function App() {
  const [status, setStatus] = React.useState<string>("Ready.");
  const [profile, setProfile] = React.useState<Profile | null>(null);

  async function loadDefaultProfile() {
    try {
      const p = await invoke<Profile>("get_default_profile");
      setProfile(p);
      setStatus("Loaded default profile from Rust backend.");
    } catch (e) {
      setStatus(`Failed to load profile: ${String(e)}`);
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <div className="logo">tap</div>
          <div className="subtitle">Timed Action Performer</div>
        </div>
        <div className="topbar-actions">
          <button className="btn" onClick={loadDefaultProfile}>
            Load default profile
          </button>
        </div>
      </header>

      <div className="layout">
        <aside className="sidebar">
          <h3>Profiles</h3>
          <div className="card">
            <div className="field">
              <div className="label">Active</div>
              <div className="value">{profile?.name ?? "(none loaded)"}</div>
            </div>
            <div className="hint">
              MVP focuses on: repeat / record / replay. Later: conditions, scripts, plugins.
            </div>
          </div>
        </aside>

        <main className="main">
          <h3>Timeline</h3>
          <div className="card">
            <div className="hint">
              This is a UI scaffold. Next steps: timeline editor, recorder panel, player controls, and a safety stop
              hotkey.
            </div>
            <div className="timeline">
              {(profile?.timeline.actions ?? []).map((a, idx) => (
                <div key={idx} className="timeline-row">
                  <div className="t">{a.at_ms}ms</div>
                  <div className="d">{a.enabled ? "enabled" : "disabled"}</div>
                </div>
              ))}
            </div>
          </div>
        </main>
      </div>

      <footer className="statusbar">
        <span className="status-label">Status</span>
        <span className="status-value">{status}</span>
      </footer>
    </div>
  );
}


