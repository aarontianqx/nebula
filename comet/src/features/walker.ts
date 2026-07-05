/**
 * 屏幕随机走动：窗口水平位移（走路帧循环由 PetCanvas 按 walk 状态播放）。
 * 全部使用物理像素坐标（Tauri outerPosition/monitor 均为物理坐标系）。
 */
import {
  currentMonitor,
  getCurrentWindow,
  PhysicalPosition,
} from "@tauri-apps/api/window";

import { hasTauri } from "../platform/env";
import type { Facing, Gait } from "../pet/types";

interface WalkHooks {
  onStart: (gait: Gait, facing: Facing) => void;
  onDone: () => void;
}

const TICK_MS = 50;
/** 走路每 tick 位移（物理 px）；Retina 2x 下约合 40 逻辑 px/s。 */
const WALK_STEP_PX = 4;
/** 奔跑每 tick 位移。 */
const RUN_STEP_PX = 15;
/** 走动中触发奔跑的概率。 */
const RUN_CHANCE = 0.25;
const MARGIN_PX = 20;

/**
 * 执行一次随机走动，返回取消函数。
 * 目标点在当前显示器内随机选取；到达或被取消后调用 onDone。
 */
export function startWalk({ onStart, onDone }: WalkHooks): () => void {
  let cancelled = false;
  let timer = 0;

  void (async () => {
    if (!hasTauri) return onDone();
    const win = getCurrentWindow();
    const [pos, size, monitor] = await Promise.all([
      win.outerPosition(),
      win.outerSize(),
      currentMonitor(),
    ]);
    if (cancelled || !monitor) return onDone();

    const minX = monitor.position.x + MARGIN_PX;
    const maxX =
      monitor.position.x + monitor.size.width - size.width - MARGIN_PX;
    if (maxX <= minX) return onDone();

    // 随机步态：奔跑走更远的距离
    const gait: Gait = Math.random() < RUN_CHANCE ? "run" : "walk";
    const step = gait === "run" ? RUN_STEP_PX : WALK_STEP_PX;
    const span =
      gait === "run" ? 400 + Math.random() * 800 : 150 + Math.random() * 450;

    let dir: Facing = Math.random() < 0.5 ? -1 : 1;
    let target = pos.x + dir * span;
    if (target < minX || target > maxX) {
      dir = -dir as Facing;
      target = Math.min(Math.max(pos.x + dir * span, minX), maxX);
    }

    let x = pos.x;
    onStart(gait, dir);
    timer = window.setInterval(() => {
      if (cancelled) return;
      x += dir * step;
      const arrived = dir > 0 ? x >= target : x <= target;
      if (arrived) {
        window.clearInterval(timer);
        onDone();
        return;
      }
      void win.setPosition(new PhysicalPosition(Math.round(x), pos.y));
    }, TICK_MS);
  })();

  return () => {
    cancelled = true;
    window.clearInterval(timer);
  };
}
