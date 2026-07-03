/**
 * 16 格姿势状态资产映射。
 * 资产来自 4×4 宫格切片（见 specs/features/pose-matrix.md）。
 */
import idle from "../assets/pet/idle.png";
import curious from "../assets/pet/curious.png";
import rest from "../assets/pet/rest.png";
import sleep from "../assets/pet/sleep.png";
import walkA from "../assets/pet/walk_a.png";
import walkB from "../assets/pet/walk_b.png";
import run from "../assets/pet/run.png";
import stretch from "../assets/pet/stretch.png";
import petted from "../assets/pet/petted.png";
import grabbed from "../assets/pet/grabbed.png";
import greet from "../assets/pet/greet.png";
import sulk from "../assets/pet/sulk.png";
import drink from "../assets/pet/drink.png";
import focus from "../assets/pet/focus.png";
import tired from "../assets/pet/tired.png";
import cheer from "../assets/pet/cheer.png";

export const POSE_SOURCES = {
  idle,
  curious,
  rest,
  sleep,
  walk_a: walkA,
  walk_b: walkB,
  run,
  stretch,
  petted,
  grabbed,
  greet,
  sulk,
  drink,
  focus,
  tired,
  cheer,
} as const;

export type Pose = keyof typeof POSE_SOURCES;

/** 待机轮换池：idle 状态下随机切换的姿势与权重。 */
export const IDLE_ROTATION: Array<{ pose: Pose; weight: number }> = [
  { pose: "idle", weight: 5 },
  { pose: "curious", weight: 2 },
  { pose: "rest", weight: 2 },
  { pose: "greet", weight: 1 },
];
