import { useState } from "react";

import { useDocumentStore } from "../stores/documentStore";
import { useUiStore } from "../stores/uiStore";

const ONBOARDED_KEY = "tap.onboarded";

export function OnboardingBanner() {
  const [done, setDone] = useState(() => localStorage.getItem(ONBOARDED_KEY) === "1");

  function dismiss(): void {
    localStorage.setItem(ONBOARDED_KEY, "1");
    setDone(true);
  }

  async function loadSample(): Promise<void> {
    await useDocumentStore.getState().applyTemplate("simple_click");
    useUiStore.getState().setMode("timeline");
    dismiss();
  }

  if (done) return null;

  return (
    <div className="card onboarding-banner">
      <div className="onboarding-head">
        <div className="onboarding-title">Welcome to tap</div>
        <button className="btn btn-small" onClick={dismiss}>
          Got it
        </button>
      </div>
      <p className="onboarding-text">
        tap records and replays timed mouse and keyboard actions. Two things to know before you start:
      </p>
      <ul className="onboarding-points">
        <li>
          <strong>Emergency stop:</strong> press <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>Backspace</kbd> at any time to
          halt a run.
        </li>
        <li>
          <strong>Try a sample</strong> to open the timeline editor with a ready-made macro.
        </li>
      </ul>
      <button className="btn btn-primary btn-small" onClick={() => void loadSample()}>
        Load a sample macro
      </button>
    </div>
  );
}
