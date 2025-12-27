import { useRef, useEffect, useState, useCallback } from "react";
import { useSessionStore } from "../../stores/sessionStore";

interface Props {
  sessionId: string;
}

export default function CanvasWindow({ sessionId }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const frame = useSessionStore((s) => s.frames[sessionId]);
  const clickSession = useSessionStore((s) => s.clickSession);
  const dragSession = useSessionStore((s) => s.dragSession);

  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState<{ x: number; y: number } | null>(
    null
  );

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

        // If moved less than 5 pixels, treat as click
        if (dx < 5 && dy < 5) {
          await clickSession(sessionId, coords.x, coords.y);
        } else {
          // Drag
          await dragSession(
            sessionId,
            dragStart.x,
            dragStart.y,
            coords.x,
            coords.y
          );
        }
      }

      setIsDragging(false);
      setDragStart(null);
    },
    [
      isDragging,
      dragStart,
      sessionId,
      clickSession,
      dragSession,
      getCanvasCoordinates,
    ]
  );

  const handleMouseLeave = useCallback(() => {
    setIsDragging(false);
    setDragStart(null);
  }, []);

  return (
    <div className="relative">
      <canvas
        ref={canvasRef}
        width={1080}
        height={720}
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
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

