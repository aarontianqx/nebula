import { useState } from "react";

import { api } from "../../lib/ipc";
import { useDocumentStore } from "../../stores/documentStore";
import { useEngineStore } from "../../stores/engineStore";
import { useLibraryStore } from "../../stores/libraryStore";
import { log } from "../../stores/logStore";

// A stable empty list: a selector must not mint a new array per call.
const NO_TAGS: string[] = [];

export function ProfilesCard() {
  const isIdle = useEngineStore((s) => s.engineState === "Idle");
  const name = useDocumentStore((s) => s.doc.name);
  const description = useDocumentStore((s) => s.doc.description) ?? "";
  const author = useDocumentStore((s) => s.doc.author) ?? "";
  const tags = useDocumentStore((s) => s.doc.tags) ?? NO_TAGS;
  const profiles = useLibraryStore((s) => s.profiles);
  const recents = useLibraryStore((s) => s.recents);
  const templates = useLibraryStore((s) => s.templates);
  const doc = useDocumentStore.getState;
  const library = useLibraryStore.getState;

  const [templateId, setTemplateId] = useState("");

  async function handleSave() {
    try {
      await doc().flush();
      await api.saveProfile(name);
      await Promise.all([library().loadProfiles(), library().loadRecents()]);
      log(`Saved: ${name}`);
    } catch (err) {
      log(`Failed to save: ${String(err)}`);
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
        <div className="field">
          <label className="label">Description</label>
          <input
            type="text"
            value={description}
            placeholder="What does this macro do?"
            onChange={(e) => doc().setDescription(e.target.value)}
            className="input"
          />
        </div>
        <div className="field-row">
          <div className="field">
            <label className="label">Author</label>
            <input type="text" value={author} onChange={(e) => doc().setAuthor(e.target.value)} className="input" />
          </div>
          <div className="field">
            <label className="label">Tags</label>
            <input
              type="text"
              value={tags.join(", ")}
              placeholder="game, login"
              onChange={(e) =>
                doc().setTags(
                  e.target.value
                    .split(",")
                    .map((t) => t.trim())
                    .filter((t) => t !== ""),
                )
              }
              className="input"
            />
          </div>
        </div>

        <div className="btn-row">
          <button className="btn btn-block" onClick={handleSave} disabled={!isIdle}>
            Save
          </button>
        </div>
        <div className="btn-row">
          <button className="btn btn-sm" onClick={() => library().importFromFile()} disabled={!isIdle}>
            Import…
          </button>
          <button className="btn btn-sm" onClick={() => library().exportToFile()} disabled={!isIdle}>
            Export…
          </button>
        </div>

        {templates.length > 0 && (
          <div className="field template-picker">
            <label className="label">New from template</label>
            <div className="field-row">
              <select
                className="input"
                value={templateId}
                onChange={(e) => setTemplateId(e.target.value)}
                disabled={!isIdle}
                title={templates.find((t) => t.id === templateId)?.description ?? ""}
              >
                <option value="">Choose a template…</option>
                {templates.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name}
                  </option>
                ))}
              </select>
              <button
                className="btn btn-sm"
                onClick={() => templateId && library().applyTemplate(templateId)}
                disabled={!isIdle || templateId === ""}
              >
                Apply
              </button>
            </div>
          </div>
        )}

        {recents.length > 0 && (
          <div className="recent-list">
            <span className="list-caption">Recent</span>
            <div className="recent-chips">
              {recents.map((r) => (
                <button key={r} className="chip" onClick={() => library().loadProfile(r)} disabled={!isIdle} title={`Load ${r}`}>
                  {r}
                </button>
              ))}
            </div>
          </div>
        )}

        {profiles.length > 0 && (
          <div className="profile-list">
            {profiles.map((p) => (
              <div key={p} className={`profile-item ${p === name ? "active" : ""}`}>
                <button className="profile-item-name" onClick={() => library().loadProfile(p)} disabled={!isIdle}>
                  {p}
                </button>
                <button
                  className="profile-item-delete"
                  title="Delete"
                  onClick={() => library().deleteProfile(p)}
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
