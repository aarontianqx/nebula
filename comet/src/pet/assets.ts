/**
 * 帧序列资产表：manifest + 帧图 URL + 播放帧率。
 *
 * 资产由 scripts/slice_frames.py 从 4×4 宫格切出（每状态 16 帧）：
 * - 同一状态所有帧共用同一裁剪窗口 → 帧间零错位；
 * - manifest.json 记录每状态的窗口尺寸与体型归一化缩放
 *   （以 idle 前景面积为基准，跨状态体型一致）。
 */
import manifest from "../assets/pet/frames/manifest.json";
import type { PetState } from "./types";

const frameUrls = import.meta.glob<string>("../assets/pet/frames/*.png", {
  eager: true,
  import: "default",
});

/**
 * 每状态播放帧率（fps）。16 帧按动作相位顺序生成，
 * 一轮即一个完整动作循环。慢状态低帧率省 CPU。
 */
const STATE_FPS: Record<PetState, number> = {
  idle: 6,
  walk: 12,
  run: 16,
  cheer: 10,
  stretch: 8,
  petted: 8,
  sleep: 4,
  rest: 5,
  drink: 8,
  focus: 5,
  tired: 6,
  sulk: 5,
  greet: 8,
  curious: 6,
  grabbed: 8,
};

export interface StateAsset {
  urls: string[];
  fps: number;
  /** 体型归一化缩放（相对 idle）。 */
  scale: number;
}

interface ManifestState {
  frames: string[];
  scale: number;
}

function resolve(state: PetState): StateAsset {
  const entry = (manifest.states as Record<string, ManifestState>)[state];
  return {
    urls: entry.frames.map((f) => {
      const url = frameUrls[`../assets/pet/frames/${f}`];
      if (!url) throw new Error(`missing frame asset: ${f}`);
      return url;
    }),
    fps: STATE_FPS[state],
    scale: entry.scale,
  };
}

export const STATE_ASSETS: Record<PetState, StateAsset> = Object.fromEntries(
  (Object.keys(STATE_FPS) as PetState[]).map((s) => [s, resolve(s)])
) as Record<PetState, StateAsset>;

/** idle 裁剪窗口高度（px），渲染端统一显示尺寸的基准。 */
export const IDLE_WINDOW_H: number = (
  manifest.states as Record<string, { h: number }>
).idle.h;

/** 播完一轮所需时长（ms），供临时状态计时回落。 */
export function cycleDuration(state: PetState): number {
  const a = STATE_ASSETS[state];
  return Math.round((a.urls.length / a.fps) * 1000);
}
