/**
 * 动态鼠标穿透 + 拖拽落地：
 * - Rust 侧全局光标事件 → 像素级命中 → 切换整窗穿透；
 * - 原生拖拽期间 WebView 收不到 mouseup，由 Rust 侧 moved 静止信号判定落地。
 */
import { useEffect, useRef } from "react";

import {
  onCursorLeft,
  onCursorPos,
  onDragEnd,
  setClickThrough,
} from "../platform/ipc";
import type { PetController } from "./usePetStateMachine";

export type HitTestFn = (x: number, y: number) => boolean;

const LANDING_MS = 500;

/** 返回 hitTestRef：由 PetCanvas 就绪时写入像素级命中函数。 */
export function useCursorPassthrough(pet: PetController) {
  const hitTestRef = useRef<HitTestFn>(() => false);

  useEffect(() => {
    const subs = [
      onCursorPos((pos) => {
        void setClickThrough(!hitTestRef.current(pos[0], pos[1]));
      }),
      onCursorLeft(() => {
        void setClickThrough(true);
      }),
      onDragEnd(() => {
        if (pet.stateRef.current === "grabbed") {
          pet.setLanding(true);
          window.setTimeout(() => pet.setLanding(false), LANDING_MS);
          pet.playTransient("petted");
        }
      }),
    ];
    return () => {
      subs.forEach((p) => p.then((un) => un()));
    };
  }, [pet]);

  return hitTestRef;
}
