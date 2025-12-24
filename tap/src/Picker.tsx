import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Position {
  x: number;
  y: number;
}

function Picker() {
  const [mousePos, setMousePos] = useState<Position>({ x: 0, y: 0 });
  const [dpiScale, setDpiScale] = useState<number>(window.devicePixelRatio || 1);

  // Update DPI scale on mount
  useEffect(() => {
    setDpiScale(window.devicePixelRatio || 1);
  }, []);

  // Handle mouse move - update crosshair position (in logical pixels for DOM)
  const handleMouseMove = useCallback((e: MouseEvent) => {
    setMousePos({ x: e.screenX, y: e.screenY });
  }, []);

  // Handle click - select position and close
  const handleClick = useCallback(async (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();

    const x = e.screenX;
    const y = e.screenY;

    try {
      await invoke("picker_position_selected", { x, y });
    } catch (err) {
      console.error("Failed to select position:", err);
    }
  }, []);

  // Handle ESC key - cancel and close
  const handleKeyDown = useCallback(async (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      try {
        await invoke("close_picker_window");
      } catch (err) {
        console.error("Failed to close picker:", err);
      }
    }
  }, []);

  useEffect(() => {
    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("click", handleClick);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("click", handleClick);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [handleMouseMove, handleClick, handleKeyDown]);

  return (
    <div className="picker-overlay">
      {/* Semi-transparent overlay */}
      <div className="picker-background" />

      {/* Crosshair */}
      <div
        className="crosshair-h"
        style={{ top: mousePos.y }}
      />
      <div
        className="crosshair-v"
        style={{ left: mousePos.x }}
      />

      {/* Coordinate display - show physical pixels for accuracy */}
      <div
        className="coord-display"
        style={{
          left: mousePos.x + 20,
          top: mousePos.y + 20
        }}
      >
        <span className="coord-value">
          ({Math.round(mousePos.x * dpiScale)}, {Math.round(mousePos.y * dpiScale)})
        </span>
        <span className="coord-hint">Click to select �?ESC to cancel</span>
      </div>

      {/* Center crosshair marker */}
      <div
        className="crosshair-center"
        style={{
          left: mousePos.x - 10,
          top: mousePos.y - 10
        }}
      />
    </div>
  );
}

export default Picker;

