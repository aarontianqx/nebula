# Proposal: Staggered Script Start & Text Input

## Overview

Two new features for Wardenly:

1. **Staggered Script Start** — Add a launch mode to "Start All" that starts scripts one-by-one with a 1-second delay between each session, skipping sessions that are already running scripts.
2. **Text Input** — Allow users to type text that gets injected into the currently focused element of a session's browser page, supporting CJK characters and cross-origin iframes.

---

## Feature 1: Staggered Script Start

### Motivation

When running "Start All" for scripts, all sessions receive the `StartScript` command simultaneously. In some game scenarios (e.g., entering a dungeon), staggering the starts avoids flooding the game server and gives each session a moment to settle before the next one begins.

### UI Design

Add a **split button** (dropdown arrow) to the existing "Start All" button in `ScriptControls`, mirroring the pattern already used by the "Run" button in the toolbar.

**Current state:**

```
[ Start All ]
```

**New state:**

```
[ Start All ▾ ]
```

Dropdown options:

| Label                 | Behavior                                                   |
|-----------------------|------------------------------------------------------------|
| **Staggered Start All** | Start scripts one-by-one, 1 second apart, skip running ones |

Design notes:
- The main "Start All" button retains its current behavior (simultaneous start).
- Clicking the dropdown arrow reveals a single option: **Staggered Start All**.
- The dropdown arrow is part of the button itself (split button), consistent with the existing Run button pattern.
- "Start All" and "Stop All" button are currently conditional — only one shows at a time based on `isRunning`. The new dropdown only appears on "Start All" (no dropdown on "Stop All").

### Backend Design

#### Adapter Layer — `commands.rs`

New Tauri command:

```rust
#[tauri::command]
pub async fn start_all_scripts_staggered(
    state: State<'_, AppState>,
    session_scripts: std::collections::HashMap<String, String>,
) -> Result<(), String> {
    state.coordinator.start_all_scripts_staggered(session_scripts).await;
    Ok(())
}
```

#### Application Layer — `coordinator.rs`

New method on `Coordinator`:

```rust
pub async fn start_all_scripts_staggered(
    &self,
    session_scripts: std::collections::HashMap<String, String>,
) {
    let sessions = self.sessions.read().await;

    // Collect eligible sessions (have a script selected, sorted deterministically)
    let mut targets: Vec<(String, String, mpsc::Sender<SessionCommand>)> = Vec::new();
    for (session_id, handle) in sessions.iter() {
        let script_name = match session_scripts.get(session_id) {
            Some(name) if !name.is_empty() => name.clone(),
            _ => continue,
        };
        // Skip if already running a script
        if handle.info.state == SessionState::ScriptRunning {
            continue;
        }
        targets.push((session_id.clone(), script_name, handle.cmd_tx.clone()));
    }
    drop(sessions); // Release read lock before spawning

    tauri::async_runtime::spawn(async move {
        for (i, (session_id, script_name, cmd_tx)) in targets.iter().enumerate() {
            if cmd_tx
                .send(SessionCommand::StartScript {
                    script_name: script_name.clone(),
                })
                .await
                .is_ok()
            {
                tracing::info!(
                    "Staggered start: script '{}' on session {} ({}/{})",
                    script_name, session_id, i + 1, targets.len()
                );
            }
            // 1-second delay between starts (except after last)
            if i < targets.len() - 1 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
        tracing::info!("Staggered script start complete");
    });
}
```

Design decisions:
- **Background task**: Like `run_group`, the staggered start is spawned as a background task so the IPC call returns immediately.
- **Skip running sessions**: Sessions already in `ScriptRunning` state are skipped entirely (no delay consumed).
- **Lock release**: The read lock on `sessions` is released before spawning the async task. The `cmd_tx` handles are cloned out.

#### Frontend — `ScriptControls.tsx`

Changes:
1. Add a `staggeredDropdownOpen` state.
2. Add a split button UI with dropdown for "Start All".
3. New handler `handleRunAllStaggered` that invokes `start_all_scripts_staggered`.

The split button only renders when `!isRunning` (the "Start All" mode). When `isRunning`, the "Stop All" button renders as-is, no dropdown.

### Registration

Register `start_all_scripts_staggered` in `lib.rs` alongside the existing `start_all_scripts`.

---

## Feature 2: Text Input (Type-to-Session)

### Motivation

The game runs inside a headless browser. Many in-game activities require text input (chat, naming, quantity entry). Since the user cannot directly interact with the browser window, we need a mechanism to inject text into the currently focused element of the game page.

### Technical Approach

#### Challenge: Cross-Origin Iframe

The game loads its actual content inside a cross-origin iframe. CDP commands like `Input.insertText` sent to the main page session will not reach elements inside a cross-origin iframe. This is because Chrome treats OOPIFs (Out-of-Process Iframes) as separate targets.

#### Solution: Dual-Strategy Text Injection

We use a **two-layer approach** that handles both same-origin and cross-origin scenarios:

**Primary strategy — CDP `Input.insertText`:**
- Sends text to whichever element currently has focus in the main page target.
- Works perfectly when the focused element is in the parent page or a same-origin iframe.
- This is the cleanest approach: no selector needed, supports full Unicode/CJK.

**Fallback strategy — JavaScript `execCommand('insertText')` via `Runtime.evaluate` on iframe target:**
- When the game runs in a cross-origin iframe, we must first attach to the iframe's CDP target session.
- Use `Target.setAutoAttach` at browser startup to automatically discover OOPIF targets.
- Identify the game iframe target, then execute `Input.insertText` within that session.
- If iframe target attachment proves unreliable, use an alternative: execute JavaScript `document.execCommand('insertText', false, text)` which inserts text at the current cursor/selection in the focused input.

**Recommended implementation order:**
1. Start with `Input.insertText` on the main page — this handles the common case (parent page inputs, same-origin).
2. Add `Target.setAutoAttach` support and OOPIF session routing — this enables cross-origin iframe text input.
3. If CDP session routing proves complex, fallback to `Runtime.evaluate` with `document.execCommand('insertText', false, text)` on the iframe context (obtained via `Page.createIsolatedWorld` or frame execution context).

### Backend Design

#### Domain Layer — `command.rs`

Add a new `SessionCommand` variant:

```rust
pub enum SessionCommand {
    // ... existing variants ...

    /// Insert text into the currently focused element
    InsertText { text: String },
}
```

#### Infrastructure Layer — `driver.rs` (BrowserDriver trait)

Add a new trait method:

```rust
#[async_trait]
pub trait BrowserDriver: Send + Sync {
    // ... existing methods ...

    /// Insert text into the currently focused element.
    /// Uses CDP Input.insertText for Unicode/CJK support.
    /// Handles cross-origin iframe targets if OOPIF session is available.
    async fn insert_text(&self, text: &str) -> anyhow::Result<()>;
}
```

#### Infrastructure Layer — `chromium.rs` (ChromiumDriver)

Implementation:

```rust
async fn insert_text(&self, text: &str) -> Result<()> {
    use chromiumoxide::cdp::browser_protocol::input::InsertTextParams;

    let page = self.page().await?;
    let page = page.lock().await;

    // Primary: insert via CDP (works for same-origin focused elements)
    page.execute(InsertTextParams::new(text)).await?;

    tracing::debug!("Inserted text ({} chars)", text.len());
    Ok(())
}
```

For Phase 2 (cross-origin iframe support), we will add OOPIF session management. This can be deferred if the game's input fields happen to receive the text via the main page session (some game engines route input events through the parent frame).

#### Application Layer — `session_actor.rs`

Handle the new command in the actor's command loop:

```rust
SessionCommand::InsertText { text } => {
    if let Err(e) = driver.insert_text(&text).await {
        tracing::error!("Failed to insert text: {}", e);
    }
}
```

#### Application Layer — `coordinator.rs`

Add methods:

```rust
/// Insert text into a specific session's focused element
pub async fn insert_text(&self, session_id: &str, text: &str) -> anyhow::Result<()> {
    let sessions = self.sessions.read().await;
    let handle = sessions
        .get(session_id)
        .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

    handle.cmd_tx
        .send(SessionCommand::InsertText { text: text.to_string() })
        .await
        .map_err(|_| anyhow::anyhow!("Failed to send insert text command"))?;

    Ok(())
}

/// Insert text into all active sessions (concurrent)
pub async fn insert_text_all(&self, text: &str) {
    let sessions = self.sessions.read().await;
    let futures: Vec<_> = sessions
        .values()
        .map(|h| h.cmd_tx.send(SessionCommand::InsertText { text: text.to_string() }))
        .collect();
    futures::future::join_all(futures).await;
}
```

#### Adapter Layer — `commands.rs`

New Tauri commands:

```rust
#[tauri::command]
pub async fn insert_text(
    state: State<'_, AppState>,
    session_id: String,
    text: String,
) -> Result<(), String> {
    state.coordinator
        .insert_text(&session_id, &text)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn insert_text_all(
    state: State<'_, AppState>,
    text: String,
) -> Result<(), String> {
    state.coordinator.insert_text_all(&text).await;
    Ok(())
}
```

### UI Design

Place the Text Input control in the **Inspector panel** (Row 2, after the existing Fetch/Click buttons), since it is a direct interaction tool for the current session.

#### Layout

```
[ X __ ] [ Y __ ] | [ Fetch ] [ Click ] | [ color-swatch  RGB(r,g,b) ]  |  [ Type... ] [ Send ]
```

**Components:**

| Element            | Description                                                           |
|--------------------|-----------------------------------------------------------------------|
| **Type...** input  | A text input field, placeholder "Type..." . Standard `<input>` with the same styling as X/Y fields but wider (min-width ~160px, flexible). |
| **Send** button    | Triggers text injection. Same style as Fetch/Click buttons. Icon: `Type` (from lucide-react). |

**Behavior:**

1. User types text into the input field.
2. Press **Enter** or click **Send** to inject:
   - If `Spread to All` is **off**: calls `insert_text` for the current session.
   - If `Spread to All` is **on**: calls `insert_text_all`.
3. After sending, the input field is **cleared** and re-focused for quick follow-up typing.
4. The input field is **disabled** when no session is selected.
5. Send button is **disabled** when input is empty or no session is selected.

**Why not a popup/dialog?**
- The Inspector panel is always visible when a session is active.
- Inline input is faster for repeated typing (chat messages).
- No extra click to open/dismiss a modal.
- Consistent with the existing Fetch/Click interaction model.

**CJK / IME support:**
- The input field is a standard HTML `<input>`, so the OS IME works naturally.
- The text is sent as a complete string to the backend (not character-by-character), so IME composition completes in the frontend before transmission.

### Registration

Register `insert_text` and `insert_text_all` in `lib.rs`.

---

## File Change Summary

| File                                        | Change                                                      |
|---------------------------------------------|-------------------------------------------------------------|
| `src-tauri/src/application/command.rs`      | Add `InsertText` variant                                    |
| `src-tauri/src/infrastructure/browser/driver.rs` | Add `insert_text` trait method                         |
| `src-tauri/src/infrastructure/browser/chromium.rs` | Implement `insert_text` using `InsertTextParams`     |
| `src-tauri/src/application/coordinator.rs`  | Add `insert_text`, `insert_text_all`, `start_all_scripts_staggered` |
| `src-tauri/src/application/service/session_actor.rs` | Handle `InsertText` command                     |
| `src-tauri/src/adapter/tauri/commands.rs`   | Add `insert_text`, `insert_text_all`, `start_all_scripts_staggered` commands |
| `src-tauri/src/lib.rs`                      | Register new commands                                       |
| `src/components/session/ScriptControls.tsx`  | Add split button dropdown for "Staggered Start All"        |
| `src/components/layout/MainWindow.tsx`       | Add Text Input field + Send button to Inspector panel      |

---

## Cross-Origin Iframe — Future Enhancement

If testing reveals that `Input.insertText` on the main page does not reach game input fields inside a cross-origin iframe, the following enhancement will be needed:

### Phase 2: OOPIF Session Management

1. **At browser startup** (`ChromiumDriver::start`): call `Target.setAutoAttach` with `flatten: true` to auto-discover iframe targets.
2. **Store iframe session**: When the `attachedToTarget` event fires for an iframe target, save the CDP session handle.
3. **Route `insert_text`**: When `insert_text` is called, execute `InsertTextParams` on the iframe's session instead of the main page session.

This would add state to `ChromiumDriver`:

```rust
pub struct ChromiumDriver {
    // ... existing fields ...
    /// CDP session for the game's cross-origin iframe (if detected)
    iframe_session: RwLock<Option<CdpSession>>,
}
```

The exact API depends on chromiumoxide's OOPIF support. If native support is insufficient, raw CDP JSON commands via `page.execute_cdp` can be used.

### Alternative: `document.execCommand` Fallback

If OOPIF session management proves too complex, a simpler fallback:

```javascript
// Execute in iframe context via Runtime.evaluate with contextId
document.execCommand('insertText', false, 'your text here');
```

This requires obtaining the iframe's execution context ID but avoids full OOPIF session management.

---

## Open Questions (Resolved)

| Question | Resolution |
|----------|-----------|
| Stagger interval configurable? | No — hardcode 1s. Configurable interval deferred to future. |
| Spread to All for text input? | Yes — concurrent send to all sessions via `insert_text_all`. |
| Text input UI location? | Inspector panel (Row 2), inline with Fetch/Click. |
| Cross-origin iframe handling? | Start with `Input.insertText` on main page. Add OOPIF support in Phase 2 if needed. |
