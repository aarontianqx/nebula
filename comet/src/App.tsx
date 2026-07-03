import { useCallback, useEffect, useRef, useState } from "react";
import { PetCanvas } from "./components/PetCanvas";
import { IDLE_ROTATION, type Pose } from "./lib/poses";
import {
  onCursorLeft,
  onCursorPos,
  onDragEnd,
  onSystemStatus,
  quitApp,
  setClickThrough,
  startDragging,
} from "./lib/ipc";
import { startWalk, type Facing } from "./lib/walker";
import { acknowledgeDrink, startHydrationScheduler } from "./lib/hydration";
import { Pomodoro, type PomodoroPhase } from "./lib/pomodoro";

/** 判定为拖拽而非点击的位移阈值（px）。 */
const DRAG_THRESHOLD = 4;
/** 双击间隔（ms）：两次点击间隔小于该值视为双击（切换番茄钟）。 */
const DOUBLE_CLICK_MS = 350;

const IDLE_POSES: readonly Pose[] = ["idle", "curious", "rest", "greet"];

function pickIdlePose(): Pose {
  const total = IDLE_ROTATION.reduce((s, e) => s + e.weight, 0);
  let r = Math.random() * total;
  for (const e of IDLE_ROTATION) {
    r -= e.weight;
    if (r <= 0) return e.pose;
  }
  return "idle";
}

export default function App() {
  const [pose, setPose] = useState<Pose>("greet");
  const [facing, setFacing] = useState<Facing>(1);
  const poseRef = useRef(pose);
  poseRef.current = pose;
  const hitTestRef = useRef<(x: number, y: number) => boolean>(() => false);
  const pressRef = useRef<{ x: number; y: number } | null>(null);
  const revertTimer = useRef<number>(0);
  const cancelWalkRef = useRef<(() => void) | null>(null);
  const remindingRef = useRef(false);
  const stressedRef = useRef(false);
  const pomodoroPhaseRef = useRef<PomodoroPhase>("off");
  const pomodoroRef = useRef<Pomodoro | null>(null);
  const lastClickAtRef = useRef(0);

  const stopWalk = useCallback(() => {
    cancelWalkRef.current?.();
    cancelWalkRef.current = null;
  }, []);

  /**
   * 常驻姿势优先级：饮水提醒 > 番茄钟阶段 > 系统高压（疲惫）> 待机。
   * 临时姿势（抚摸/欢呼等）结束后回落到该姿势。
   */
  const basePose = useCallback((): Pose => {
    if (remindingRef.current) return "drink";
    if (pomodoroPhaseRef.current === "focus") return "focus";
    if (pomodoroPhaseRef.current === "break") return "rest";
    if (stressedRef.current) return "tired";
    return "idle";
  }, []);

  /** 播放一个临时姿势，结束后回落到常驻姿势。 */
  const playTransient = useCallback(
    (p: Pose, ms: number) => {
      stopWalk();
      window.clearTimeout(revertTimer.current);
      setPose(p);
      revertTimer.current = window.setTimeout(() => setPose(basePose()), ms);
    },
    [stopWalk, basePose]
  );

  // 待机行为调度：每 8~16s 在「换待机姿势 / 随机走动」之间选择。
  // 仅在常驻姿势为待机（无提醒/番茄钟）且当前处于待机姿势时行动。
  useEffect(() => {
    let timer = 0;
    const schedule = () => {
      timer = window.setTimeout(() => {
        if (basePose() === "idle" && IDLE_POSES.includes(poseRef.current)) {
          if (Math.random() < 0.35) {
            setPose("walk_a");
            cancelWalkRef.current = startWalk({
              onFrame: (p, f) => {
                setPose(p);
                setFacing(f);
              },
              onDone: () => {
                cancelWalkRef.current = null;
                setPose(basePose());
              },
            });
          } else {
            setPose(pickIdlePose());
          }
        }
        schedule();
      }, 8000 + Math.random() * 8000);
    };
    schedule();
    return () => {
      window.clearTimeout(timer);
      stopWalk();
    };
  }, [stopWalk, basePose]);

  // 健康饮水提醒：到点切换 drink 姿势常驻，点击宠物确认后欢呼并重新计时
  useEffect(() => {
    return startHydrationScheduler(() => {
      remindingRef.current = true;
      stopWalk();
      window.clearTimeout(revertTimer.current);
      setPose("drink");
    });
  }, [stopWalk]);

  // 番茄钟：双击切换。专注期 focus 姿势，完成欢呼，休息期 rest 姿势
  useEffect(() => {
    const pomodoro = new Pomodoro({
      onPhase: (phase) => {
        pomodoroPhaseRef.current = phase;
        stopWalk();
        window.clearTimeout(revertTimer.current);
        setPose(basePose());
      },
      onFocusDone: () => playTransient("cheer", 2000),
    });
    pomodoroRef.current = pomodoro;
    pomodoro.restore();
    return () => pomodoro.dispose();
  }, [stopWalk, basePose, playTransient]);

  // 系统状态联动：CPU 高负载或低电量（未充电）→ tired 姿势（带迟滞防抖动）
  useEffect(() => {
    const sub = onSystemStatus((s) => {
      const lowBattery = s.battery !== null && s.battery < 20 && s.charging === false;
      // 迟滞：进入 85%，退出 65%，避免在阈值附近来回切换
      const next = stressedRef.current
        ? s.cpu > 65 || lowBattery
        : s.cpu > 85 || lowBattery;
      if (next === stressedRef.current) return;
      stressedRef.current = next;
      // 仅在待机类姿势时立即体现，避免打断交互/走动/提醒
      const passive =
        IDLE_POSES.includes(poseRef.current) || poseRef.current === "tired";
      if (passive && !cancelWalkRef.current) setPose(basePose());
    });
    return () => {
      void sub.then((un) => un());
    };
  }, [basePose]);

  // 全局光标 → 像素级命中 → 动态穿透
  useEffect(() => {
    const subs = [
      onCursorPos((pos) => {
        void setClickThrough(!hitTestRef.current(pos[0], pos[1]));
      }),
      onCursorLeft(() => {
        void setClickThrough(true);
      }),
      // 原生拖拽期间 WebView 收不到 mouseup，由 Rust 侧通知拖拽结束
      onDragEnd(() => {
        if (poseRef.current === "grabbed") playTransient("petted", 900);
      }),
    ];
    return () => {
      subs.forEach((p) => p.then((un) => un()));
    };
  }, [playTransient]);

  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    pressRef.current = { x: e.screenX, y: e.screenY };
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    const press = pressRef.current;
    if (!press) return;
    const moved =
      Math.abs(e.screenX - press.x) > DRAG_THRESHOLD ||
      Math.abs(e.screenY - press.y) > DRAG_THRESHOLD;
    if (moved) {
      pressRef.current = null;
      stopWalk();
      window.clearTimeout(revertTimer.current);
      setPose("grabbed");
      void startDragging();
    }
  };

  const handleMouseUp = () => {
    if (!pressRef.current) return;
    pressRef.current = null;

    const now = Date.now();
    const isDouble = now - lastClickAtRef.current < DOUBLE_CLICK_MS;
    lastClickAtRef.current = now;

    if (isDouble) {
      // 双击：切换番茄钟（开始专注 / 取消当前会话）
      const phase = pomodoroRef.current?.toggle();
      if (phase === "focus") playTransient("greet", 800);
      return;
    }

    if (remindingRef.current) {
      // 点击视为“已喝水”：欢呼致谢并重新计时
      remindingRef.current = false;
      acknowledgeDrink();
      playTransient("cheer", 1500);
    } else {
      playTransient("petted", 1200);
    }
  };

  const walking = pose === "walk_a" || pose === "walk_b";

  return (
    <div className="w-full h-full flex items-end justify-center">
      <div
        className={pose === "grabbed" ? "pet-grabbed" : "pet-idle"}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onContextMenu={(e) => {
          e.preventDefault();
          void quitApp();
        }}
      >
        <PetCanvas
          pose={pose}
          // 走路素材朝右；向左走时水平镜像
          flip={walking && facing === -1}
          onHitTestReady={(fn) => (hitTestRef.current = fn)}
        />
      </div>
    </div>
  );
}
