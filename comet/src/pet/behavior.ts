/** 待机行为策略：轮换池与待机类状态判定。 */
import type { PetState } from "./types";

/** 待机类状态：行为调度器可在这些状态时切换行为（换姿势/走动）。 */
export const IDLE_STATES: readonly PetState[] = [
  "idle",
  "curious",
  "rest",
  "stretch",
  "sleep",
  "greet",
];

/** 待机轮换池：idle 状态下随机切换的状态与权重。 */
const IDLE_ROTATION: Array<{ state: PetState; weight: number }> = [
  { state: "idle", weight: 5 },
  { state: "curious", weight: 2 },
  { state: "rest", weight: 2 },
  { state: "stretch", weight: 1 },
  { state: "sleep", weight: 1 },
  { state: "greet", weight: 1 },
];

export function pickIdleState(): PetState {
  const total = IDLE_ROTATION.reduce((s, e) => s + e.weight, 0);
  let r = Math.random() * total;
  for (const e of IDLE_ROTATION) {
    r -= e.weight;
    if (r <= 0) return e.state;
  }
  return "idle";
}
