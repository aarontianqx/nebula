/**
 * Tauri IPC 封装：命令调用与 Rust 侧事件订阅。
 * 所有 API 在非 Tauri 环境（浏览器直开）下空操作降级。
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { hasTauri } from "./env";

/** 切换整窗鼠标穿透。带去重：状态未变化时不发 IPC。 */
let lastIgnore: boolean | null = null;
export async function setClickThrough(ignore: boolean): Promise<void> {
  if (!hasTauri || lastIgnore === ignore) return;
  lastIgnore = ignore;
  await invoke("set_click_through", { ignore });
}

export async function quitApp(): Promise<void> {
  if (!hasTauri) return;
  await invoke("quit");
}

export async function startDragging(): Promise<void> {
  if (!hasTauri) return;
  await getCurrentWindow().startDragging();
}

/** 订阅 Rust 侧全局光标位置（窗口内逻辑坐标）。 */
export function onCursorPos(
  handler: (pos: [number, number]) => void
): Promise<UnlistenFn> {
  if (!hasTauri) return Promise.resolve(() => {});
  return listen<[number, number]>("cursor-pos", (e) => handler(e.payload));
}

export function onCursorLeft(handler: () => void): Promise<UnlistenFn> {
  if (!hasTauri) return Promise.resolve(() => {});
  return listen("cursor-left", handler);
}

export interface SystemStatus {
  /** 全局 CPU 使用率 0~100。 */
  cpu: number;
  /** 电池电量 0~100；无电池为 null。 */
  battery: number | null;
  /** 是否在充电/接电源；无电池为 null。 */
  charging: boolean | null;
}

/** 订阅 Rust 侧系统状态采样（10s 一次）。 */
export function onSystemStatus(
  handler: (status: SystemStatus) => void
): Promise<UnlistenFn> {
  if (!hasTauri) return Promise.resolve(() => {});
  return listen<SystemStatus>("system-status", (e) => handler(e.payload));
}

/**
 * 拖拽结束检测：原生拖拽期间 WebView 收不到 mouseup，
 * 以窗口 moved 事件停止 200ms 作为拖拽结束信号。
 */
export function onDragEnd(handler: () => void): Promise<UnlistenFn> {
  if (!hasTauri) return Promise.resolve(() => {});
  let timer = 0;
  return getCurrentWindow().onMoved(() => {
    window.clearTimeout(timer);
    timer = window.setTimeout(handler, 200);
  });
}
