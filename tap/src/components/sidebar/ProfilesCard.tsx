import { api } from "../../lib/ipc";
import { useDocumentStore } from "../../stores/documentStore";
import { useEngineStore } from "../../stores/engineStore";

export function ProfilesCard() {
  const isIdle = useEngineStore((s) => s.engineState === "Idle");
  const name = useDocumentStore((s) => s.name);
  const profiles = useDocumentStore((s) => s.profiles);
  const doc = useDocumentStore.getState;

  async function handleSave() {
    try {
      // Persist the latest in-memory edits, not a stale backend copy.
      await doc().flush();
      await api.saveProfile(name);
      await doc().loadProfiles();
      useEngineStore.getState().addLog(`Saved: ${name}`);
    } catch (err) {
      useEngineStore.getState().addLog(`Failed to save: ${String(err)}`);
    }
  }

  return (
    <>
      <h3>Profiles</h3>
      <div className="card">
        <div className="field">
          <label className="label">Name</label>
          <input type="text" value={name} onChange={(e) => doc().setName(e.target.value)} className="input" />
        </div>
        <button className="btn btn-block" onClick={handleSave} disabled={!isIdle}>
          Save
        </button>
        {profiles.length > 0 && (
          <div className="profile-list">
            {profiles.map((p) => (
              <div key={p} className={`profile-item ${p === name ? "active" : ""}`}>
                <button className="profile-item-name" onClick={() => doc().loadProfile(p)} disabled={!isIdle}>
                  {p}
                </button>
                <button
                  className="profile-item-delete"
                  title="Delete"
                  onClick={() => doc().deleteProfile(p)}
                  disabled={!isIdle}
                >
                  ×
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </>
  );
}
