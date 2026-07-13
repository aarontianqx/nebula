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
  walk: 10,
  run: 12,
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
  kind: "sequence" | "pose" | "keyframes";
  /** Variable hold time for semantic key poses. */
  durations?: number[];
  blink?: boolean;
  decorator?: "glasses" | "hearts" | "zzz" | "question";
  /** 体型归一化缩放（相对 idle）。 */
  scale: number;
}

interface ManifestState {
  frames: string[];
  scale: number;
}

interface StateDefinition {
  kind: StateAsset["kind"];
  source?: PetState;
  frames?: number[];
  durations?: number[];
  blink?: boolean;
  decorator?: StateAsset["decorator"];
}

/**
 * Migration map for the consistency-first animation protocol.
 *
 * Micro-motion states use one canonical pose. Semantic actions retain only
 * reviewed key poses. Locomotion and interactions that genuinely require
 * continuous limb motion remain frame sequences.
 */
const STATE_DEFINITIONS: Record<PetState, StateDefinition> = {
  idle: { kind: "pose", frames: [0, 1], blink: true },
  walk: { kind: "sequence", frames: [6, 7, 8, 9, 10, 11] },
  run: { kind: "sequence", frames: [12, 13, 14, 15] },
  cheer: {
    kind: "keyframes",
    frames: [0, 1, 5, 8, 12, 15],
    durations: [180, 140, 220, 140, 180, 220],
  },
  stretch: { kind: "sequence" },
  petted: {
    kind: "keyframes",
    frames: [0, 1, 2, 3, 2, 1],
    durations: [140, 170, 220, 280, 180, 160],
    decorator: "hearts",
  },
  sleep: { kind: "pose", frames: [0], decorator: "zzz" },
  rest: { kind: "pose", frames: [0] },
  drink: { kind: "sequence" },
  focus: {
    kind: "pose",
    source: "idle",
    frames: [0, 1],
    blink: true,
    decorator: "glasses",
  },
  tired: { kind: "pose", frames: [0] },
  sulk: { kind: "pose", frames: [0] },
  greet: {
    kind: "keyframes",
    frames: [0, 2, 4, 6, 9, 15],
    durations: [160, 150, 180, 180, 180, 220],
  },
  curious: { kind: "pose", frames: [0], decorator: "question" },
  grabbed: { kind: "sequence" },
};

function resolve(state: PetState): StateAsset {
  const definition = STATE_DEFINITIONS[state];
  const source = definition.source ?? state;
  const entry = (manifest.states as Record<string, ManifestState>)[source];
  const indices = definition.frames ?? entry.frames.map((_, index) => index);
  return {
    urls: indices.map((index) => {
      const f = entry.frames[index];
      const url = frameUrls[`../assets/pet/frames/${f}`];
      if (!url) throw new Error(`missing frame asset: ${f}`);
      return url;
    }),
    fps: STATE_FPS[state],
    kind: definition.kind,
    durations: definition.durations,
    blink: definition.blink,
    decorator: definition.decorator,
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
  if (a.durations) return a.durations.reduce((total, duration) => total + duration, 0);
  return Math.round((a.urls.length / a.fps) * 1000);
}
