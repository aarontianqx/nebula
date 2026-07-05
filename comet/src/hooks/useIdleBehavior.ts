/**
 * 待机行为调度：每 10~20s 在「换待机状态 / 随机走动」之间选择。
 * 仅在常驻状态为待机（无提醒/番茄钟/高压）且当前处于待机类状态时行动。
 */
import { useEffect } from "react";

import { startWalk } from "../features/walker";
import { IDLE_STATES, pickIdleState } from "../pet/behavior";
import type { PetController } from "./usePetStateMachine";

const MIN_INTERVAL_MS = 10_000;
const JITTER_MS = 10_000;
/** 行动时选择走动（而非换姿势）的概率。 */
const WALK_CHANCE = 0.3;

export function useIdleBehavior(pet: PetController): void {
  useEffect(() => {
    let timer = 0;
    const schedule = () => {
      timer = window.setTimeout(() => {
        if (
          pet.baseState() === "idle" &&
          IDLE_STATES.includes(pet.stateRef.current)
        ) {
          if (Math.random() < WALK_CHANCE) {
            pet.cancelWalkRef.current = startWalk({
              onStart: (gait, facing) => {
                pet.setFacing(facing);
                pet.setState(gait);
              },
              onDone: () => {
                pet.cancelWalkRef.current = null;
                pet.setState(pet.baseState());
              },
            });
          } else {
            pet.setState(pickIdleState());
          }
        }
        schedule();
      }, MIN_INTERVAL_MS + Math.random() * JITTER_MS);
    };
    schedule();
    return () => {
      window.clearTimeout(timer);
      pet.stopWalk();
    };
  }, [pet]);
}
