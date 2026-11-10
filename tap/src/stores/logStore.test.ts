import { beforeEach, describe, expect, it } from "vitest";

import { clearLog, log, useLogStore } from "./logStore";

beforeEach(clearLog);

describe("activity log", () => {
  it("keeps the newest 200 entries", () => {
    for (let i = 0; i < 250; i++) log(`line ${i}`);

    const { logs } = useLogStore.getState();
    expect(logs).toHaveLength(200);
    expect(logs[0].message).toBe("line 50");
    expect(logs.at(-1)?.message).toBe("line 249");
  });

  it("stamps entries with a millisecond clock time and can be cleared", () => {
    log("hello");
    expect(useLogStore.getState().logs[0].time).toMatch(/^\d\d:\d\d:\d\d\.\d{3}$/);

    clearLog();
    expect(useLogStore.getState().logs).toEqual([]);
  });
});
