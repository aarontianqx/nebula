/**
 * 屏幕随机走动：walk_a/walk_b 交替 + 窗口水平位移。
 * 全部使用物理像素坐标（Tauri outerPosition/monitor 均为物理坐标系）。
 */
import {
  currentMonitor,
  getCurrentWindow,
  PhysicalPosition,
} from "@tauri-apps/api/window";
import type { Pose } from "./poses";

export type Facing = 1 | -1;

interface WalkHooks {
  onFrame: (pose: Pose, facing: Facing) => void;
  onDone: () => void;
}

const TICK_MS = 50;
/** 每 tick 位移（物理 px）；Retina 2x 下约合 40 逻辑 px/s。 */
const STEP_PX = 4;
/** 走路两帧交替周期（tick 数）。 */
const FRAME_TICKS = 4;
const MARGIN_PX = 20;

const hasTauri = "__TAURI_INTERNALS__" in window;

/**
 * 执行一次随机走动，返回取消函数。
 * 目标点在当前显示器内随机选取；到达或被取消后调用 onDone。
 */
export function startWalk({ onFrame, onDone }: WalkHooks): () => void {
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

    // 随机目标：至少走 150px，方向随机，越界则反向
    const span = 150 + Math.random() * 450;
    let dir: Facing = Math.random() < 0.5 ? -1 : 1;
    let target = pos.x + dir * span;
    if (target < minX || target > maxX) {
      dir = -dir as Facing;
      target = Math.min(Math.max(pos.x + dir * span, minX), maxX);
    }

    let x = pos.x;
    let tick = 0;
    timer = window.setInterval(() => {
      if (cancelled) return;
      x += dir * STEP_PX;
      const arrived = dir > 0 ? x >= target : x <= target;
      if (arrived) {
        window.clearInterval(timer);
        onDone();
        return;
      }
      void win.setPosition(new PhysicalPosition(Math.round(x), pos.y));
      onFrame(
        Math.floor(tick / FRAME_TICKS) % 2 === 0 ? "walk_a" : "walk_b",
        dir
      );
      tick += 1;
    }, TICK_MS);
  })();

  return () => {
    cancelled = true;
    window.clearInterval(timer);
  };
}
