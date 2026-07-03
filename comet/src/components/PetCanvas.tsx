import { useEffect, useRef } from "react";
import type { Pose } from "../lib/poses";
import { POSE_SOURCES } from "../lib/poses";

/** 画布逻辑尺寸。 */
const W = 240;
const H = 250;

/**
 * 统一参考比例：切片按内容紧裁后尺寸各异，
 * 以原始宫格单元 (384×256) 为参考系缩放，保证各姿势体型一致。
 */
const REF_W = 384;
const REF_H = 256;
const SCALE = Math.min(W / REF_W, H / REF_H);

interface Props {
  pose: Pose;
  /** 水平镜像（素材朝右，向左移动时翻转）。 */
  flip?: boolean;
  /** 向上层暴露像素级命中函数（alpha > 0 视为命中宠物本体）。 */
  onHitTestReady: (fn: (x: number, y: number) => boolean) => void;
}

const imageCache = new Map<Pose, HTMLImageElement>();

function loadPose(pose: Pose): Promise<HTMLImageElement> {
  const cached = imageCache.get(pose);
  if (cached?.complete) return Promise.resolve(cached);
  return new Promise((resolve, reject) => {
    const img = cached ?? new Image();
    if (!cached) {
      img.src = POSE_SOURCES[pose];
      imageCache.set(pose, img);
    }
    img.onload = () => resolve(img);
    img.onerror = reject;
  });
}

/** 序列帧宠物：绘制当前姿势贴图，仅姿势切换时重绘（无常驻渲染循环）。 */
export function PetCanvas({ pose, flip = false, onHitTestReady }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current!;
    const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
    onHitTestReady((x, y) => {
      const r = canvas.getBoundingClientRect();
      const px = Math.floor(((x - r.left) / r.width) * canvas.width);
      const py = Math.floor(((y - r.top) / r.height) * canvas.height);
      if (px < 0 || py < 0 || px >= canvas.width || py >= canvas.height) {
        return false;
      }
      return ctx.getImageData(px, py, 1, 1).data[3] > 10;
    });
  }, [onHitTestReady]);

  useEffect(() => {
    let stale = false;
    void loadPose(pose).then((img) => {
      if (stale) return;
      const canvas = canvasRef.current!;
      const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
      ctx.clearRect(0, 0, W, H);
      // 统一参考系缩放，锚定底部居中（贴地）
      const dw = img.width * SCALE;
      const dh = img.height * SCALE;
      ctx.save();
      if (flip) {
        ctx.translate(W, 0);
        ctx.scale(-1, 1);
      }
      ctx.drawImage(img, (W - dw) / 2, H - dh, dw, dh);
      ctx.restore();
    });
    return () => {
      stale = true;
    };
  }, [pose, flip]);

  return (
    <canvas
      ref={canvasRef}
      width={W}
      height={H}
      style={{ width: W, height: H, display: "block" }}
    />
  );
}
