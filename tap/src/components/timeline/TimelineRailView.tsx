import { useRef, useState } from "react";

import { actionGlyph, formatAction } from "../../lib/actions";
import { useDocumentStore } from "../../stores/documentStore";
import { useEngineStore } from "../../stores/engineStore";
import { useUiStore } from "../../stores/uiStore";

export function TimelineRailView() {
  const isIdle = useEngineStore((s) => s.engineState === "Idle");
  const timeline = useDocumentStore((s) => s.timeline);
  const editable = useDocumentStore((s) => s.editable);
  const doc = useDocumentStore.getState;
  const selectedIdx = useUiStore((s) => s.selectedActionIdx);
  const selectAction = useUiStore((s) => s.selectAction);

  const trackRef = useRef<HTMLDivElement>(null);
  const scaleRef = useRef<number>(1);
  const [dragIdx, setDragIdx] = useState<number | null>(null);

  const canEdit = isIdle && editable;
  const maxAt = timeline.reduce((m, a) => Math.max(m, a.at_ms), 0);
  const scale = Math.max(maxAt, 1);

  if (timeline.length === 0) {
    return (
      <div className="card timeline-card">
        <div className="timeline-empty">No actions yet. Record or import a macro to see the rail.</div>
      </div>
    );
  }

  function msFromClientX(clientX: number): number {
    const rect = trackRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return 0;
    const frac = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
    return Math.round(frac * scaleRef.current);
  }

  return (
    <div className="card timeline-rail-card">
      <div className="rail-scale">
        <span>0ms</span>
        <span>{maxAt}ms</span>
      </div>
      <div
        className="rail-track"
        ref={trackRef}
        onPointerMove={(e) => {
          if (dragIdx === null) return;
          doc().setAtMs(dragIdx, msFromClientX(e.clientX));
        }}
        onPointerUp={() => setDragIdx(null)}
        onPointerLeave={() => setDragIdx(null)}
      >
        <div className="rail-line" />
        {timeline.map((item, idx) => (
          <button
            key={idx}
            className={`rail-marker ${!item.enabled ? "disabled" : ""} ${selectedIdx === idx ? "selected" : ""} ${
              canEdit ? "draggable" : ""
            }`}
            style={{ left: `${(item.at_ms / scale) * 100}%` }}
            title={`${item.at_ms}ms — ${formatAction(item.action)}`}
            onClick={() => selectAction(idx)}
            onPointerDown={(e) => {
              if (!canEdit) return;
              scaleRef.current = scale;
              setDragIdx(idx);
              selectAction(idx);
              (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
            }}
          >
            <span className="rail-glyph">{actionGlyph(item.action)}</span>
            <span className="rail-marker-time">{item.at_ms}</span>
          </button>
        ))}
      </div>
      {canEdit && <div className="rail-hint">Drag markers to retime. Select a marker to edit it in the Inspector.</div>}
    </div>
  );
}
