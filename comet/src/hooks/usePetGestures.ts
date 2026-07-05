/**
 * 宠物交互手势：单击（抚摸/确认饮水）、双击（番茄钟）、拖拽（拎起）。
 * 返回绑定到宠物容器元素的鼠标事件处理器。
 */
import { useRef } from "react";

import { acknowledgeDrink } from "../features/hydration";
import type { Pomodoro } from "../features/pomodoro";
import { startDragging } from "../platform/ipc";
import type { PetController } from "./usePetStateMachine";

/** 判定为拖拽而非点击的位移阈值（px）。 */
const DRAG_THRESHOLD = 4;
/** 双击间隔（ms）：两次点击间隔小于该值视为双击（切换番茄钟）。 */
const DOUBLE_CLICK_MS = 350;

export function usePetGestures(
  pet: PetController,
  pomodoroRef: React.RefObject<Pomodoro | null>
) {
  const pressRef = useRef<{ x: number; y: number } | null>(null);
  const lastClickAtRef = useRef(0);

  const onMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    pressRef.current = { x: e.screenX, y: e.screenY };
  };

  const onMouseMove = (e: React.MouseEvent) => {
    const press = pressRef.current;
    if (!press) return;
    const moved =
      Math.abs(e.screenX - press.x) > DRAG_THRESHOLD ||
      Math.abs(e.screenY - press.y) > DRAG_THRESHOLD;
    if (moved) {
      pressRef.current = null;
      pet.interrupt();
      pet.setState("grabbed");
      void startDragging();
    }
  };

  const onMouseUp = () => {
    if (!pressRef.current) return;
    pressRef.current = null;

    const now = Date.now();
    const isDouble = now - lastClickAtRef.current < DOUBLE_CLICK_MS;
    lastClickAtRef.current = now;

    if (isDouble) {
      // 双击：切换番茄钟（开始专注 / 取消当前会话）
      const phase = pomodoroRef.current?.toggle();
      if (phase === "focus") pet.playTransient("greet");
      return;
    }

    if (pet.remindingRef.current) {
      // 点击视为"已喝水"：欢呼致谢并重新计时
      pet.remindingRef.current = false;
      acknowledgeDrink();
      pet.playTransient("cheer");
    } else {
      pet.playTransient("petted");
    }
  };

  return { onMouseDown, onMouseMove, onMouseUp };
}
