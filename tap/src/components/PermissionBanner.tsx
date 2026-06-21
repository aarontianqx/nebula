import { useState } from "react";

import { usePermissionStore } from "../stores/permissionStore";

const WIN_HINT_KEY = "tap.winAdminHintDismissed";

export function PermissionBanner() {
  const os = usePermissionStore((s) => s.os);
  const accessibility = usePermissionStore((s) => s.accessibility);
  const screenRecording = usePermissionStore((s) => s.screen_recording);
  const refresh = usePermissionStore((s) => s.refresh);
  const requestScreen = usePermissionStore((s) => s.requestScreenRecording);
  const openSettings = usePermissionStore((s) => s.openSettings);

  const [winHintDismissed, setWinHintDismissed] = useState(
    () => localStorage.getItem(WIN_HINT_KEY) === "1",
  );

  if (os === "macos") {
    if (accessibility && screenRecording) return null;
    return (
      <div className="card permission-banner">
        <div className="permission-head">
          <span className="permission-icon">!</span>
          <div className="permission-headtext">
            <div className="permission-title">Permissions needed</div>
            <div className="permission-sub">macOS must allow tap to control input and read the screen.</div>
          </div>
          <button className="btn btn-small" onClick={() => void refresh()}>
            Re-check
          </button>
        </div>
        <ul className="permission-list">
          {!accessibility && (
            <li>
              <span>
                <strong>Accessibility</strong> — required to record and replay clicks and keys.
              </span>
              <button className="btn btn-small" onClick={() => void openSettings("accessibility")}>
                Open Settings
              </button>
            </li>
          )}
          {!screenRecording && (
            <li>
              <span>
                <strong>Screen Recording</strong> — required for the color picker, pixel conditions, and window titles.
              </span>
              <div className="btn-row">
                <button className="btn btn-small" onClick={() => void requestScreen()}>
                  Request
                </button>
                <button className="btn btn-small" onClick={() => void openSettings("screen_recording")}>
                  Open Settings
                </button>
              </div>
            </li>
          )}
        </ul>
        <div className="permission-foot">After granting, restart tap so the new permissions take effect.</div>
      </div>
    );
  }

  if (os === "windows" && !winHintDismissed) {
    return (
      <div className="card permission-banner permission-hint">
        <div className="permission-head">
          <span className="permission-icon">i</span>
          <div className="permission-headtext">
            <div className="permission-title">Running on Windows</div>
            <div className="permission-sub">
              If clicks or keys don&apos;t reach an elevated app (e.g. a game launched as admin), run tap as
              administrator too.
            </div>
          </div>
          <button
            className="btn btn-small"
            onClick={() => {
              localStorage.setItem(WIN_HINT_KEY, "1");
              setWinHintDismissed(true);
            }}
          >
            Got it
          </button>
        </div>
      </div>
    );
  }

  return null;
}
