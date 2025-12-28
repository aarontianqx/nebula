import { useRef, useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "../../stores/sessionStore";

interface Props {
  sessionId: string;
  onCanvasClick?: (x: number, y: number) => void;
  onMouseAction?: (action: "click" | "drag", x: number, y: number, endX?: number, endY?: number) => void;
}

export default function CanvasWindow({ sessionId, onCanvasClick, onMouseAction }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const frame = useSessionStore((s) => s.frames[sessionId]);

  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState<{ x: number; y: number } | null>(null);

  // Draw frame to canvas
  useEffect(() => {
    if (!frame || !canvasRef.current) return;

    const ctx = canvasRef.current.getContext("2d");
    if (!ctx) return;

    const img = new Image();
    img.onload = () => {
      ctx.drawImage(img, 0, 0, canvasRef.current!.width, canvasRef.current!.height);
    };
    img.src = `data:image/jpeg;base64,${frame}`;
  }, [frame]);

  const getCanvasCoordinates = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const rect = canvasRef.current?.getBoundingClientRect();
      if (!rect) return null;

      // Scale to actual viewport coordinates (1080x720)
      const scaleX = 1080 / rect.width;
      const scaleY = 720 / rect.height;

      return {
        x: (e.clientX - rect.left) * scaleX,
        y: (e.clientY - rect.top) * scaleY,
      };
    },
    []
  );

  const handleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const coords = getCanvasCoordinates(e);
      if (coords) {
        setIsDragging(true);
        setDragStart(coords);
      }
    },
    [getCanvasCoordinates]
  );

  const handleMouseUp = useCallback(
    async (e: React.MouseEvent<HTMLCanvasElement>) => {
      const coords = getCanvasCoordinates(e);
      if (!coords) return;

      if (isDragging && dragStart) {
        const dx = Math.abs(coords.x - dragStart.x);
        const dy = Math.abs(coords.y - dragStart.y);

        // Always notify parent of the click position for inspector
        onCanvasClick?.(coords.x, coords.y);

        // If moved less than 5 pixels, treat as click
        if (dx < 5 && dy < 5) {
          onMouseAction?.("click", coords.x, coords.y);
        } else {
          // Drag
          onMouseAction?.("drag", dragStart.x, dragStart.y, coords.x, coords.y);
        }
      }

      setIsDragging(false);
      setDragStart(null);
    },
    [isDragging, dragStart, onCanvasClick, onMouseAction, getCanvasCoordinates]
  );

  const handleMouseLeave = useCallback(() => {
    setIsDragging(false);
    setDragStart(null);
    // Notify backend that cursor left canvas
    invoke("update_cursor_position", { x: 0, y: 0, inBounds: false });
  }, []);

  // Throttled cursor position update for keyboard passthrough
  const lastUpdateRef = useRef<number>(0);
  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const now = Date.now();
      if (now - lastUpdateRef.current < 50) return; // Throttle to 50ms
      lastUpdateRef.current = now;

      const coords = getCanvasCoordinates(e);
      if (coords) {
        invoke("update_cursor_position", {
          x: Math.round(coords.x),
          y: Math.round(coords.y),
          inBounds: true,
        });
      }
    },
    [getCanvasCoordinates]
  );

  return (
    <div className="relative">
      <canvas
        ref={canvasRef}
        width={1080}
        height={720}
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
        className="w-full max-w-[1080px] border border-[var(--color-border)] rounded cursor-crosshair bg-black"
        style={{ aspectRatio: "1080 / 720" }}
      />
      {!frame && (
        <div className="absolute inset-0 flex items-center justify-center text-[var(--color-text-muted)]">
          Waiting for screencast...
        </div>
      )}
    </div>
  );
}
