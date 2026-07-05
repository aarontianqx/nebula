/**
 * 番茄钟：专注期 → 完成庆祝 → 休息期 → 回到待机。
 * 进行中的会话持久化在 localStorage，应用重启可恢复剩余计时。
 */
import { readJSON, readMinutes, writeJSON } from "../platform/storage";

export type PomodoroPhase = "off" | "focus" | "break";

const SESSION_KEY = "comet.pomodoro";
const FOCUS_MIN_KEY = "comet.pomodoroFocusMin";
const BREAK_MIN_KEY = "comet.pomodoroBreakMin";
const DEFAULT_FOCUS_MIN = 25;
const DEFAULT_BREAK_MIN = 5;
const TICK_MS = 1000;

interface Session {
  phase: Exclude<PomodoroPhase, "off">;
  /** 当前阶段结束时刻（epoch ms）。 */
  endsAt: number;
}

function loadSession(): Session | null {
  const s = readJSON<Session>(SESSION_KEY);
  if (s && (s.phase === "focus" || s.phase === "break") && s.endsAt > Date.now()) {
    return s;
  }
  return null;
}

export interface PomodoroHooks {
  onPhase: (phase: PomodoroPhase) => void;
  /** 专注期正常结束时触发（庆祝时机）。 */
  onFocusDone: () => void;
}

export class Pomodoro {
  private session: Session | null = null;
  private timer = 0;

  constructor(private hooks: PomodoroHooks) {}

  /** 恢复重启前未结束的会话；无会话则保持 off。 */
  restore(): void {
    this.session = loadSession();
    if (this.session) {
      this.hooks.onPhase(this.session.phase);
      this.schedule();
    }
  }

  get phase(): PomodoroPhase {
    return this.session?.phase ?? "off";
  }

  /** off → focus；进行中则取消。返回新阶段。 */
  toggle(): PomodoroPhase {
    if (this.session) {
      this.stop();
    } else {
      this.session = {
        phase: "focus",
        endsAt:
          Date.now() + readMinutes(FOCUS_MIN_KEY, DEFAULT_FOCUS_MIN) * 60_000,
      };
      writeJSON(SESSION_KEY, this.session);
      this.hooks.onPhase("focus");
      this.schedule();
    }
    return this.phase;
  }

  stop(): void {
    window.clearTimeout(this.timer);
    this.session = null;
    writeJSON(SESSION_KEY, null);
    this.hooks.onPhase("off");
  }

  dispose(): void {
    window.clearTimeout(this.timer);
  }

  private schedule(): void {
    window.clearTimeout(this.timer);
    this.timer = window.setTimeout(() => this.tick(), TICK_MS);
  }

  private tick(): void {
    if (!this.session) return;
    if (Date.now() < this.session.endsAt) {
      this.schedule();
      return;
    }
    if (this.session.phase === "focus") {
      this.session = {
        phase: "break",
        endsAt:
          Date.now() + readMinutes(BREAK_MIN_KEY, DEFAULT_BREAK_MIN) * 60_000,
      };
      writeJSON(SESSION_KEY, this.session);
      this.hooks.onFocusDone();
      this.hooks.onPhase("break");
      this.schedule();
    } else {
      this.stop();
    }
  }
}
