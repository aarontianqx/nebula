/**
 * 健康饮水提醒调度。
 * 上次饮水时间持久化在 localStorage，应用重启不重置计时。
 */
import { readMinutes, readTimestamp } from "../platform/storage";

const LAST_DRINK_KEY = "comet.lastDrinkAt";
const INTERVAL_KEY = "comet.drinkIntervalMin";
const DEFAULT_INTERVAL_MIN = 45;
const CHECK_MS = 30_000;

/** 确认已饮水，重新开始计时。 */
export function acknowledgeDrink(): void {
  localStorage.setItem(LAST_DRINK_KEY, String(Date.now()));
}

/**
 * 启动调度：到点后调用 onRemind（每个周期只触发一次，
 * 直到 acknowledgeDrink 重置）。返回停止函数。
 */
export function startHydrationScheduler(onRemind: () => void): () => void {
  // 首次运行从现在起算，避免装机即提醒
  if (readTimestamp(LAST_DRINK_KEY) === 0) acknowledgeDrink();

  let remindedFor = 0;
  const check = () => {
    const last = readTimestamp(LAST_DRINK_KEY);
    const interval = readMinutes(INTERVAL_KEY, DEFAULT_INTERVAL_MIN) * 60_000;
    if (Date.now() - last >= interval && remindedFor !== last) {
      remindedFor = last;
      onRemind();
    }
  };
  check();
  const timer = window.setInterval(check, CHECK_MS);
  return () => window.clearInterval(timer);
}
