import { useEngineStore } from "../../stores/engineStore";
import { useToolStore } from "../../stores/toolStore";

export function ColorPickerCard() {
  const mousePos = useEngineStore((s) => s.mousePos);
  const pickedColor = useToolStore((s) => s.pickedColor);
  const tool = useToolStore.getState;

  return (
    <>
      <h3>Color Picker</h3>
      <div className="card">
        <button className="btn btn-block" onClick={() => tool().pickColorAtCursor()} disabled={!mousePos}>
          Pick Color at Cursor
        </button>
        {pickedColor && (
          <div className="color-preview">
            <div className="color-swatch" style={{ backgroundColor: pickedColor.hex }} />
            <span className="color-value">{pickedColor.hex}</span>
            <span className="color-rgb">
              ({pickedColor.r}, {pickedColor.g}, {pickedColor.b})
            </span>
          </div>
        )}
      </div>
    </>
  );
}
