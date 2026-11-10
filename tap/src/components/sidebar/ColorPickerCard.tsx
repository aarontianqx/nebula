import { useColorStore } from "../../stores/colorStore";
import { useCursorStore } from "../../stores/cursorStore";

export function ColorPickerCard() {
  const hasCursor = useCursorStore((s) => s.pos !== null);
  const pickedColor = useColorStore((s) => s.picked);

  return (
    <>
      <h3>Color Picker</h3>
      <div className="card">
        <button className="btn btn-block" onClick={() => useColorStore.getState().pickAtCursor()} disabled={!hasCursor}>
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
