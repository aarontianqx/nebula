import type { MouseEvent } from "react";

import { formatDslAction } from "../../lib/actions";
import { isEnabled } from "../../lib/document";
import { useDocumentStore } from "../../stores/documentStore";
import { useEngineStore } from "../../stores/engineStore";
import { useUiStore } from "../../stores/uiStore";

export function TimelineListView() {
  const isIdle = useEngineStore((s) => s.engineState === "Idle");
  const timeline = useDocumentStore((s) => s.doc.timeline);
  const doc = useDocumentStore.getState;
  const selectedIdx = useUiStore((s) => s.selectedActionIdx);
  const selectAction = useUiStore((s) => s.selectAction);

  const canEdit = isIdle;

  if (timeline.length === 0) {
    return (
      <div className="card timeline-card">
        <div className="timeline-empty">
          No actions yet. Record actions, add one from the toolbar, or import a YAML file.
        </div>
      </div>
    );
  }

  return (
    <div className="card timeline-card">
      <div className="timeline-list">
        {timeline.map((item, idx) => (
          <div
            key={idx}
            className={`timeline-item ${!isEnabled(item) ? "disabled" : ""} ${selectedIdx === idx ? "selected" : ""}`}
            onClick={() => selectAction(idx)}
          >
            <input
              type="checkbox"
              className="timeline-enable"
              checked={isEnabled(item)}
              onChange={(e) => {
                e.stopPropagation();
                doc().toggleAction(idx);
              }}
              disabled={!canEdit}
              title={isEnabled(item) ? "Disable" : "Enable"}
            />
            <span className="timeline-time">{item.at_ms}ms</span>
            <span className="timeline-action">
              {formatDslAction(item.action)}
              {item.note && <span className="timeline-note"> — {item.note}</span>}
            </span>
            {canEdit && (
              <div className="timeline-actions">
                <button className="btn btn-sm" title="-50ms" onClick={(e) => stop(e, () => doc().adjustDelay(idx, -50))}>
                  -
                </button>
                <button className="btn btn-sm" title="+50ms" onClick={(e) => stop(e, () => doc().adjustDelay(idx, 50))}>
                  +
                </button>
                <button className="btn btn-sm" title="Move up" onClick={(e) => stop(e, () => doc().moveAction(idx, -1))}>
                  {"\u2191"}
                </button>
                <button className="btn btn-sm" title="Move down" onClick={(e) => stop(e, () => doc().moveAction(idx, 1))}>
                  {"\u2193"}
                </button>
                <button className="btn btn-sm" title="Duplicate" onClick={(e) => stop(e, () => doc().duplicateAction(idx))}>
                  {"\u29C9"}
                </button>
                <button
                  className="btn btn-sm btn-danger"
                  title="Delete"
                  onClick={(e) =>
                    stop(e, () => {
                      doc().deleteAction(idx);
                      if (selectedIdx === idx) selectAction(null);
                    })
                  }
                >
                  {"\u00D7"}
                </button>
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

function stop(e: MouseEvent, fn: () => void): void {
  e.stopPropagation();
  fn();
}
