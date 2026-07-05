/**
 * 健康与效率功能挂钩：饮水提醒、番茄钟、系统状态联动。
 * 均通过 PetController 驱动姿态，互不感知。
 */
import { useEffect, useRef } from "react";

import { startHydrationScheduler } from "../features/hydration";
import { Pomodoro } from "../features/pomodoro";
import { onSystemStatus } from "../platform/ipc";
import { IDLE_STATES } from "../pet/behavior";
import type { PetController } from "./usePetStateMachine";

/** 饮水提醒：到点切 drink 常驻；确认逻辑在手势层（点击 = 已喝水）。 */
export function useHydration(pet: PetController): void {
  useEffect(() => {
    return startHydrationScheduler(() => {
      pet.remindingRef.current = true;
      pet.applyBase();
    });
  }, [pet]);
}

/** 番茄钟：返回实例引用供手势层 toggle。专注期 focus、休息期 rest、完成欢呼。 */
export function usePomodoro(pet: PetController) {
  const pomodoroRef = useRef<Pomodoro | null>(null);

  useEffect(() => {
    const pomodoro = new Pomodoro({
      onPhase: (phase) => {
        pet.pomodoroPhaseRef.current = phase;
        pet.applyBase();
      },
      onFocusDone: () => pet.playTransient("cheer"),
    });
    pomodoroRef.current = pomodoro;
    pomodoro.restore();
    return () => pomodoro.dispose();
  }, [pet]);

  return pomodoroRef;
}

/** 系统状态联动：CPU 高负载或低电量（未充电）→ tired（带迟滞防抖动）。 */
export function useSystemStress(pet: PetController): void {
  useEffect(() => {
    const sub = onSystemStatus((s) => {
      const lowBattery =
        s.battery !== null && s.battery < 20 && s.charging === false;
      // 迟滞：进入 85%，退出 65%，避免在阈值附近来回切换
      const next = pet.stressedRef.current
        ? s.cpu > 65 || lowBattery
        : s.cpu > 85 || lowBattery;
      if (next === pet.stressedRef.current) return;
      pet.stressedRef.current = next;
      // 仅在待机类状态时立即体现，避免打断交互/走动/提醒
      const passive =
        IDLE_STATES.includes(pet.stateRef.current) ||
        pet.stateRef.current === "tired";
      if (passive && !pet.cancelWalkRef.current) pet.setState(pet.baseState());
    });
    return () => {
      void sub.then((un) => un());
    };
  }, [pet]);
}
