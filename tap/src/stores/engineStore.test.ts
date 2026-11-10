import { beforeEach, describe, expect, it, vi } from "vitest";

import type { EngineEvent } from "../lib/types";
import { freshStores } from "../test/harness";

vi.mock("../lib/ipc", async () => (await import("../test/ipcMock")).createIpcMock());

type Stores = Awaited<ReturnType<typeof freshStores>>;

let stores: Stores;

beforeEach(async () => {
  stores = await freshStores();
});

function state() {
  return stores.useEngineStore.getState();
}

function emit(event: EngineEvent): void {
  state().handleEngineEvent(event);
}

function messages(): string[] {
  return stores.useLogStore.getState().logs.map((l) => l.message);
}

describe("state transitions", () => {
  it("tracks the engine state and reflects it in the status line", () => {
    emit({ StateChanged: { old: "Idle", new: "Arming" } });
    expect(state().engineState).toBe("Arming");
    expect(state().status).toBe("Arming...");

    emit({ StateChanged: { old: "Arming", new: "Running" } });
    expect(state().status).toBe("Running");

    emit({ StateChanged: { old: "Running", new: "Paused" } });
    expect(state().status).toBe("Paused");
    expect(messages()).toContain("State: Running -> Paused");
  });

  it("returning to Idle clears the countdown, last action and status", () => {
    emit({ CountdownTick: { remaining_secs: 2 } });
    emit({ ActionStarting: { index: 0, action: { Wait: { ms: 5 } } } });
    expect(state().countdown).toBe(2);
    expect(state().lastAction).toBe("Wait 5ms");

    emit({ StateChanged: { old: "Running", new: "Idle" } });

    expect(state().countdown).toBeNull();
    expect(state().lastAction).toBeNull();
    expect(state().status).toBe("Ready");
  });
});

describe("run progress", () => {
  it("shows the countdown", () => {
    emit({ CountdownTick: { remaining_secs: 3 } });
    expect(state().countdown).toBe(3);
    expect(state().status).toBe("Starting in 3...");
  });

  it("describes the running action and counts completed ones", () => {
    emit({ ActionStarting: { index: 0, action: { Click: { x: 1, y: 2, button: "Left" } } } });
    expect(state().lastAction).toBe("Click @ (1, 2)");
    expect(state().status).toBe("Executing: Click @ (1, 2)");

    emit({ ActionCompleted: { index: 0 } });
    emit({ ActionCompleted: { index: 1 } });
    expect(state().executedCount).toBe(2);
  });

  it("records iterations and completion", () => {
    emit({ IterationCompleted: { iteration: 4 } });
    expect(state().iteration).toBe(4);
    expect(messages()).toContain("Iteration #4");

    emit("Completed");
    expect(state().status).toBe("Completed");
    expect(messages()).toContain("All done");
  });
});

describe("conditions, counters and target window", () => {
  it("logs condition and counter events", () => {
    emit({ WaitingForCondition: { condition: "px" } });
    expect(state().status).toBe("Waiting: px");
    emit({ ConditionSatisfied: { condition: "px" } });
    emit({ ConditionTimeout: { condition: "px" } });
    emit({ CounterChanged: { key: "i", value: 3 } });

    expect(messages()).toEqual(
      expect.arrayContaining(["Waiting: px", "Condition met: px", "Timeout: px", "i = 3"]),
    );
  });

  it("tracks target-window focus", () => {
    emit({ TargetWindowUnfocused: { title: "Game", process: null } });
    expect(state().targetWindowMatched).toBe(false);
    expect(state().status).toBe("Target window not focused");

    emit("TargetWindowFocused");
    expect(state().targetWindowMatched).toBe(true);
    expect(state().status).toBe("Running");
  });
});

describe("errors and unknown events", () => {
  it("surfaces engine errors in the status and the log", () => {
    emit({ Error: { message: "boom" } });
    expect(state().status).toBe("Error: boom");
    expect(messages()).toContain("Error: boom");
  });

  it("does not throw on events it does not know", () => {
    emit("SomethingNew" as unknown as EngineEvent);
    emit({ SomethingElse: { x: 1 } } as unknown as EngineEvent);

    expect(messages()[0]).toBe("(unknown engine event) SomethingNew");
    expect(messages()[1]).toContain("(unknown engine event)");
  });
});

describe("run stats", () => {
  it("resetRunStats clears counters and the log", () => {
    emit({ ActionCompleted: { index: 0 } });
    emit({ IterationCompleted: { iteration: 2 } });
    state().resetRunStats();

    expect(state().executedCount).toBe(0);
    expect(state().iteration).toBe(0);
    expect(messages()).toEqual([]);
  });
});

describe("lifecycle controls", () => {
  it("logs success after the backend call resolves", async () => {
    stores.api.pauseExecution.mockResolvedValue(undefined);
    await state().pause();
    expect(stores.api.pauseExecution).toHaveBeenCalledTimes(1);
    expect(messages()).toContain("Paused");
  });

  it("reports a failed call in the status and the log instead of throwing", async () => {
    stores.api.stopExecution.mockRejectedValue("not running");
    await expect(state().stop()).resolves.toBeUndefined();
    expect(state().status).toBe("Failed: not running");
    expect(messages()).toContain("Error: not running");
  });

  it("routes resume and emergency stop to their own commands", async () => {
    await state().resume();
    await state().emergencyStop();
    expect(stores.api.resumeExecution).toHaveBeenCalledTimes(1);
    expect(stores.api.emergencyStop).toHaveBeenCalledTimes(1);
    expect(messages()).toEqual(expect.arrayContaining(["Resumed", "Emergency stop requested"]));
  });

  it("onEmergencyStop flags the status", () => {
    state().onEmergencyStop();
    expect(state().status).toBe("Emergency stopped!");
    expect(messages()).toContain("EMERGENCY STOP");
  });
});
