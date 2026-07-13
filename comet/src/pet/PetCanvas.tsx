import { useEffect, useRef } from "react";

import { IDLE_WINDOW_H, STATE_ASSETS } from "./assets";
import type { PetState } from "./types";

/** 画布逻辑尺寸（需容纳最宽/最高状态的缩放帧，见 fit 保护）。 */
const W = 200;
const H = 190;

/**
 * 统一显示高度：idle 站姿高约占画布 85%，
 * 其余状态先按各自窗口等比放置，再乘 manifest 的体型归一化 scale。
 */
const BASE_SCALE = (H * 0.85) / IDLE_WINDOW_H;

interface Props {
  state: PetState;
  /** 水平镜像（素材朝右，向左移动时翻转）。 */
  flip?: boolean;
  /** 向上层暴露像素级命中函数（alpha > 0 视为命中宠物本体）。 */
  onHitTestReady: (fn: (x: number, y: number) => boolean) => void;
}

const imageCache = new Map<string, HTMLImageElement>();

const IDLE_BLINK_MIN_MS = 2_400;
const IDLE_BLINK_JITTER_MS = 3_100;
const IDLE_BLINK_HOLD_MS = 120;

function loadFrame(url: string): Promise<HTMLImageElement> {
  const cached = imageCache.get(url);
  if (cached?.complete) return Promise.resolve(cached);
  return new Promise((resolve, reject) => {
    const img = cached ?? new Image();
    if (!cached) {
      img.src = url;
      imageCache.set(url, img);
    }
    img.onload = () => resolve(img);
    img.onerror = reject;
  });
}

/**
 * 帧序列播放器：按状态循环播放切片帧。
 * - 同状态帧共用裁剪窗口（切片保证），绘制参数恒定 → 帧间零错位；
 * - setInterval 按状态 fps 驱动，慢状态低频率省 CPU；
 * - 帧图预加载完成后才开始播放，避免闪烁。
 */
export function PetCanvas({ state, flip = false, onHitTestReady }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const hitCanvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current!;
    const hitCanvas = document.createElement("canvas");
    hitCanvas.width = W;
    hitCanvas.height = H;
    hitCanvasRef.current = hitCanvas;
    onHitTestReady((x, y) => {
      const r = canvas.getBoundingClientRect();
      const px = Math.floor(((x - r.left) / r.width) * canvas.width);
      const py = Math.floor(((y - r.top) / r.height) * canvas.height);
      if (px < 0 || py < 0 || px >= canvas.width || py >= canvas.height) {
        return false;
      }
      const hitContext = hitCanvasRef.current?.getContext("2d", {
        willReadFrequently: true,
      });
      return (hitContext?.getImageData(px, py, 1, 1).data[3] ?? 0) > 10;
    });
    return () => {
      hitCanvasRef.current = null;
    };
  }, [onHitTestReady]);

  useEffect(() => {
    const asset = STATE_ASSETS[state];
    let stale = false;
    let timer = 0;

    void Promise.all(asset.urls.map(loadFrame)).then((frames) => {
      if (stale) return;
      const canvas = canvasRef.current!;
      const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
      const hitContext = hitCanvasRef.current!.getContext("2d")!;

      // 状态内所有帧同尺寸（共用裁剪窗口），绘制参数一次算好。
      // fit 保护：体型归一化后仍超出画布的状态整体再缩，杜绝削头断尾。
      let scale = BASE_SCALE * asset.scale;
      const fit = Math.min(
        1,
        W / (frames[0].width * scale),
        H / (frames[0].height * scale)
      );
      scale *= fit;
      const dw = frames[0].width * scale;
      const dh = frames[0].height * scale;
      const dx = (W - dw) / 2;
      const dy = H - dh; // 贴地

      let idx = 0;
      const drawDecorator = (frameIndex: number) => {
        if (!asset.decorator) return;
        const styles = getComputedStyle(canvas);
        const ink = styles.getPropertyValue("--pet-ink-color").trim();
        const effect = styles.getPropertyValue("--pet-effect-color").trim();

        if (asset.decorator === "hearts") {
          if (frameIndex === 0 || frameIndex === frames.length - 1) return;
          const drawHeart = (x: number, y: number, size: number) => {
            ctx.beginPath();
            ctx.moveTo(x, y + size * 0.28);
            ctx.bezierCurveTo(x - size, y - size * 0.35, x - size * 0.42, y - size, x, y - size * 0.35);
            ctx.bezierCurveTo(x + size * 0.42, y - size, x + size, y - size * 0.35, x, y + size * 0.28);
            ctx.fill();
          };
          ctx.save();
          ctx.fillStyle = effect;
          drawHeart(dx + dw * 0.08, dy + dh * 0.24, 7);
          drawHeart(dx + dw * 0.91, dy + dh * 0.18, 6);
          ctx.restore();
          return;
        }

        if (asset.decorator === "zzz" || asset.decorator === "question") {
          ctx.save();
          ctx.fillStyle = ink;
          ctx.font = `600 ${Math.max(16, dw * 0.14)}px system-ui`;
          ctx.fillText(
            asset.decorator === "zzz" ? "Zzz" : "?",
            dx + dw * 0.74,
            dy + dh * 0.2
          );
          ctx.restore();
          return;
        }

        const lensY = dy + dh * 0.34;
        const lensRadius = dw * 0.115;
        const leftX = dx + dw * 0.35;
        const rightX = dx + dw * 0.65;
        ctx.save();
        ctx.strokeStyle = ink;
        ctx.lineWidth = Math.max(2, dw * 0.018);
        ctx.beginPath();
        ctx.arc(leftX, lensY, lensRadius, 0, Math.PI * 2);
        ctx.arc(rightX, lensY, lensRadius, 0, Math.PI * 2);
        ctx.moveTo(leftX + lensRadius, lensY);
        ctx.lineTo(rightX - lensRadius, lensY);
        ctx.stroke();
        ctx.restore();
      };

      const drawFrame = (frame: HTMLImageElement, frameIndex = 0) => {
        ctx.clearRect(0, 0, W, H);
        hitContext.clearRect(0, 0, W, H);
        ctx.save();
        hitContext.save();
        if (flip) {
          ctx.translate(W, 0);
          ctx.scale(-1, 1);
          hitContext.translate(W, 0);
          hitContext.scale(-1, 1);
        }
        ctx.drawImage(frame, dx, dy, dw, dh);
        hitContext.drawImage(frame, dx, dy, dw, dh);
        ctx.restore();
        hitContext.restore();
        drawDecorator(frameIndex);
      };

      const draw = () => {
        drawFrame(frames[idx], idx);
        idx = (idx + 1) % frames.length;
      };

      if (asset.blink) {
        // Idle 使用一张已验收标准底图。眨眼只从闭眼参考帧覆盖双眼局部，
        // 避免为微动作循环重绘整只宠物造成脸型、毛发和体型闪烁。
        const base = frames[0];
        const closedEyesReference = frames[1];
        const eyePatch = {
          sx: base.width * 0.17,
          sy: base.height * 0.19,
          sw: base.width * 0.66,
          sh: base.height * 0.23,
        };
        const drawIdle = (eyesClosed: boolean) => {
          drawFrame(base, 0);
          if (!eyesClosed) return;
          ctx.drawImage(
            closedEyesReference,
            eyePatch.sx,
            eyePatch.sy,
            eyePatch.sw,
            eyePatch.sh,
            dx + eyePatch.sx * scale,
            dy + eyePatch.sy * scale,
            eyePatch.sw * scale,
            eyePatch.sh * scale
          );
          drawDecorator(0);
        };

        const scheduleBlink = () => {
          timer = window.setTimeout(() => {
            if (stale) return;
            drawIdle(true);
            timer = window.setTimeout(() => {
              if (stale) return;
              drawIdle(false);
              scheduleBlink();
            }, IDLE_BLINK_HOLD_MS);
          }, IDLE_BLINK_MIN_MS + Math.random() * IDLE_BLINK_JITTER_MS);
        };

        drawIdle(false);
        scheduleBlink();
        return;
      }

      draw();
      if (asset.kind === "keyframes" && asset.durations) {
        const durations = asset.durations;
        const scheduleKeyframe = () => {
          timer = window.setTimeout(() => {
            if (stale) return;
            draw();
            scheduleKeyframe();
          }, durations[(idx + durations.length - 1) % durations.length]);
        };
        scheduleKeyframe();
      } else if (asset.kind === "sequence" && frames.length > 1) {
        timer = window.setInterval(draw, 1000 / asset.fps);
      }
    });

    return () => {
      stale = true;
      window.clearInterval(timer);
    };
  }, [state, flip]);

  return (
    <canvas
      ref={canvasRef}
      width={W}
      height={H}
      style={{ width: W, height: H, display: "block" }}
    />
  );
}
