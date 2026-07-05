/**
 * 组装层：状态机 + 各行为挂钩 + 渲染。
 * 业务逻辑分别位于 hooks/（行为挂钩）、features/（功能模块）、pet/（宠物域）。
 */
import { PetCanvas } from "./pet/PetCanvas";
import { quitApp } from "./platform/ipc";
import { useCursorPassthrough } from "./hooks/useCursorPassthrough";
import { useIdleBehavior } from "./hooks/useIdleBehavior";
import { usePetGestures } from "./hooks/usePetGestures";
import { usePetStateMachine } from "./hooks/usePetStateMachine";
import { useHydration, usePomodoro, useSystemStress } from "./hooks/useWellness";

export default function App() {
  const { state, facing, landing, controller } = usePetStateMachine();

  useIdleBehavior(controller);
  useHydration(controller);
  const pomodoroRef = usePomodoro(controller);
  useSystemStress(controller);
  const hitTestRef = useCursorPassthrough(controller);
  const gestures = usePetGestures(controller, pomodoroRef);

  // 程序化微动画：拖拽钟摆 / 落地弹性 / 待机呼吸（styles/globals.css）
  const motionClass =
    state === "grabbed" ? "pet-grabbed" : landing ? "pet-land" : "pet-idle";

  return (
    <div className="w-full h-full flex items-end justify-center">
      <div
        className={motionClass}
        onMouseDown={gestures.onMouseDown}
        onMouseMove={gestures.onMouseMove}
        onMouseUp={gestures.onMouseUp}
        onContextMenu={(e) => {
          e.preventDefault();
          void quitApp();
        }}
      >
        <PetCanvas
          state={state}
          // 走路/奔跑素材朝右；向左移动时水平镜像
          flip={(state === "walk" || state === "run") && facing === -1}
          onHitTestReady={(fn) => (hitTestRef.current = fn)}
        />
      </div>
    </div>
  );
}
