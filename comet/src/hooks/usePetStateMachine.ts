/**
 * 宠物状态机核心：当前姿态 + 常驻状态优先级 + 临时状态回落。
 *
 * controller 是稳定引用（useMemo），供各行为挂钩（调度/提醒/手势）
 * 读写共享标志与切换状态，避免 App 层 prop 层层传递。
 */
import { useCallback, useMemo, useRef, useState } from "react";

import type { PomodoroPhase } from "../features/pomodoro";
import { cycleDuration } from "../pet/assets";
import type { Facing, PetState } from "../pet/types";

export interface PetController {
  /** 当前状态的实时引用（事件回调中读取，避免闭包过期）。 */
  stateRef: React.MutableRefObject<PetState>;
  setState(s: PetState): void;
  setFacing(f: Facing): void;
  setLanding(v: boolean): void;

  /** 常驻状态优先级：饮水提醒 > 番茄钟阶段 > 系统高压 > 待机。 */
  baseState(): PetState;
  /** 停走 + 清临时计时 + 立即回落常驻状态。 */
  applyBase(): void;
  /** 播放一个临时状态，结束后回落常驻状态。时长默认取播完整轮。 */
  playTransient(s: PetState, ms?: number): void;
  /** 停走 + 清临时回落计时（切入需长期保持的状态前调用，如拖拽）。 */
  interrupt(): void;
  /** 取消进行中的随机走动（若有）。 */
  stopWalk(): void;

  /** 进行中的走动取消函数；由待机调度器写入。 */
  cancelWalkRef: React.MutableRefObject<(() => void) | null>;
  /** 饮水提醒待确认。 */
  remindingRef: React.MutableRefObject<boolean>;
  /** 系统高压（CPU/低电量）。 */
  stressedRef: React.MutableRefObject<boolean>;
  /** 番茄钟当前阶段。 */
  pomodoroPhaseRef: React.MutableRefObject<PomodoroPhase>;
}

export function usePetStateMachine() {
  const [state, setState] = useState<PetState>("greet");
  const [facing, setFacing] = useState<Facing>(1);
  const [landing, setLanding] = useState(false);

  const stateRef = useRef(state);
  stateRef.current = state;
  const revertTimer = useRef(0);
  const cancelWalkRef = useRef<(() => void) | null>(null);
  const remindingRef = useRef(false);
  const stressedRef = useRef(false);
  const pomodoroPhaseRef = useRef<PomodoroPhase>("off");

  const stopWalk = useCallback(() => {
    cancelWalkRef.current?.();
    cancelWalkRef.current = null;
  }, []);

  const baseState = useCallback((): PetState => {
    if (remindingRef.current) return "drink";
    if (pomodoroPhaseRef.current === "focus") return "focus";
    if (pomodoroPhaseRef.current === "break") return "rest";
    if (stressedRef.current) return "tired";
    return "idle";
  }, []);

  const interrupt = useCallback(() => {
    stopWalk();
    window.clearTimeout(revertTimer.current);
  }, [stopWalk]);

  const applyBase = useCallback(() => {
    interrupt();
    setState(baseState());
  }, [interrupt, baseState]);

  const playTransient = useCallback(
    (s: PetState, ms?: number) => {
      interrupt();
      setState(s);
      revertTimer.current = window.setTimeout(
        () => setState(baseState()),
        ms ?? cycleDuration(s)
      );
    },
    [interrupt, baseState]
  );

  const controller = useMemo<PetController>(
    () => ({
      stateRef,
      setState,
      setFacing,
      setLanding,
      baseState,
      applyBase,
      playTransient,
      interrupt,
      stopWalk,
      cancelWalkRef,
      remindingRef,
      stressedRef,
      pomodoroPhaseRef,
    }),
    [baseState, applyBase, playTransient, interrupt, stopWalk]
  );

  return { state, facing, landing, controller };
}
