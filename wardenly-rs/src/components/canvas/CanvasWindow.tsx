import { useRef, useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "../../stores/sessionStore";

interface Props {
  sessionId: string;
  onCanvasClick?: (x: number, y: number) => void;
  onMouseAction?: (action: "click" | "drag", x: number, y: number, endX?: number, endY?: number) => void;
  keyboardPassthrough?: boolean;
  spreadToAll?: boolean;
}

export default function CanvasWindow({ sessionId, onCanvasClick, onMouseAction, keyboardPassthrough, spreadToAll }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  // Use the single currentFrame instead of looking up by sessionId
  const frame = useSessionStore((s) => s.currentFrame);

  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState<{ x: number; y: number } | null>(null);

  // Keyboard passthrough state
  const [cursorInBounds, setCursorInBounds] = useState(false);
  const [currentCursorPos, setCurrentCursorPos] = useState<{ x: number; y: number } | null>(null);
  const activeKeyRef = useRef<string | null>(null);
  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const repeatIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Keyboard gesture configuration
  const [keyboardConfig, setKeyboardConfig] = useState({
    longPressThresholdMs: 300,
    repeatIntervalMs: 100,
  });

  // Load keyboard configuration on mount
  useEffect(() => {
    invoke<{ long_press_threshold_ms: number; repeat_interval_ms: number }>("get_keyboard_config")
      .then((config) => {
        setKeyboardConfig({
          longPressThresholdMs: config.long_press_threshold_ms,
          repeatIntervalMs: config.repeat_interval_ms,
        });
      })
      .catch((err) => {
        console.warn("Failed to load keyboard config, using defaults:", err);
      });
  }, []);

  // Draw frame to canvas, or clear it when frame is undefined
  useEffect(() => {
    if (!canvasRef.current) return;

    const ctx = canvasRef.current.getContext("2d");
    if (!ctx) return;

    if (!frame) {
      // Clear canvas when no frame (prevents stale content)
      ctx.clearRect(0, 0, canvasRef.current.width, canvasRef.current.height);
      return;
    }

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
    setCursorInBounds(false);
    setCurrentCursorPos(null);
  }, []);

  // Track cursor position for keyboard passthrough
  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const coords = getCanvasCoordinates(e);
      if (coords) {
        setCursorInBounds(true);
        setCurrentCursorPos(coords);
      }
    },
    [getCanvasCoordinates]
  );

  // Auto-focus canvas when mouse enters (if keyboard passthrough is enabled)
  const handleMouseEnter = useCallback(() => {
    if (keyboardPassthrough && canvasRef.current) {
      canvasRef.current.focus();
    }
  }, [keyboardPassthrough]);

  // Helper function to check if key is A-Z
  const isAZKey = useCallback((key: string): boolean => {
    return key.length === 1 && /^[a-zA-Z]$/.test(key);
  }, []);

  // Helper function to trigger click
  const triggerClick = useCallback(
    async (x: number, y: number) => {
      try {
        if (spreadToAll) {
          await invoke("click_all_sessions", { x, y });
        } else {
          await invoke("click_session", { sessionId, x, y });
        }
      } catch (error) {
        console.error("Failed to trigger keyboard click:", error);
      }
    },
    [sessionId, spreadToAll]
  );

  // Cleanup function for timers
  const cleanupTimers = useCallback(() => {
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
    if (repeatIntervalRef.current) {
      clearInterval(repeatIntervalRef.current);
      repeatIntervalRef.current = null;
    }
  }, []);

  // Keyboard event handlers
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLCanvasElement>) => {
      // Only process if keyboard passthrough is enabled
      if (!keyboardPassthrough) return;

      // Only process A-Z keys
      if (!isAZKey(e.key)) return;

      // Ignore if cursor is not in canvas bounds
      if (!cursorInBounds || !currentCursorPos) return;

      // Ignore if key is already pressed (prevent repeat from OS)
      if (activeKeyRef.current === e.key) return;

      // Prevent default to avoid any browser shortcuts
      e.preventDefault();

      // Mark key as active
      activeKeyRef.current = e.key;

      // Trigger immediate click (Tap gesture start)
      triggerClick(currentCursorPos.x, currentCursorPos.y);

      // Set up long press timer (configurable threshold)
      longPressTimerRef.current = setTimeout(() => {
        // Start repeating clicks (configurable interval)
        repeatIntervalRef.current = setInterval(() => {
          if (currentCursorPos) {
            triggerClick(currentCursorPos.x, currentCursorPos.y);
          }
        }, keyboardConfig.repeatIntervalMs);
      }, keyboardConfig.longPressThresholdMs);
    },
    [keyboardPassthrough, isAZKey, cursorInBounds, currentCursorPos, triggerClick, keyboardConfig]
  );

  const handleKeyUp = useCallback(
    (e: React.KeyboardEvent<HTMLCanvasElement>) => {
      // Only process if this was the active key
      if (activeKeyRef.current !== e.key) return;

      // Prevent default
      e.preventDefault();

      // Clear active key and timers
      activeKeyRef.current = null;
      cleanupTimers();
    },
    [cleanupTimers]
  );

  // Cleanup on unmount or when keyboard passthrough is disabled
  useEffect(() => {
    return () => {
      cleanupTimers();
      activeKeyRef.current = null;
    };
  }, [cleanupTimers]);

  useEffect(() => {
    if (!keyboardPassthrough) {
      cleanupTimers();
      activeKeyRef.current = null;
    }
  }, [keyboardPassthrough, cleanupTimers]);

  return (
    <div className="relative">
      <canvas
        ref={canvasRef}
        width={1080}
        height={720}
        tabIndex={0}
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
        onMouseMove={handleMouseMove}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        onKeyDown={handleKeyDown}
        onKeyUp={handleKeyUp}
        className="w-full max-w-[1080px] border border-[var(--color-border)] rounded cursor-crosshair bg-black outline-none"
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
