package presentation

import (
	"context"
	"image"
	"log/slog"
	"sync"
	"time"

	"wardenly-go/core/command"
	"wardenly-go/core/event"
	"wardenly-go/core/state"
	"wardenly-go/domain/account"
	"wardenly-go/domain/group"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

// MainWindow is the main application window.
type MainWindow struct {
	window            fyne.Window
	canvasManager     *CanvasManager
	screencastManager *ScreencastManager
	bridge            *UIEventBridge
	logger            *slog.Logger

	// UI components - Sidebar layout
	sessionList *SessionList
	detailPanel *fyne.Container
	emptyDetail fyne.CanvasObject

	// UI components - Toolbar
	accountSelect *widget.Select
	groupSelect   *widget.Select
	runAccountBtn *widget.Button
	runGroupBtn   *widget.Button
	manageBtn     *widget.Button
	spreadToAllCb *widget.Check
	autoRefreshCb *widget.Check

	// Data
	accounts         []*account.Account
	groups           []*group.Group
	scriptNames      []string
	sessionMap       map[string]*SessionTab
	sessionMapMu     sync.RWMutex
	currentSessionID string

	// Cleanup
	cleanupOnce sync.Once

	// Services
	accountService *account.Service
	groupService   *group.Service
}

// MainWindowConfig holds configuration for MainWindow.
type MainWindowConfig struct {
	App            fyne.App
	Bridge         *UIEventBridge
	Logger         *slog.Logger
	AccountService *account.Service
	GroupService   *group.Service
	ScriptNames    []string
}

// NewMainWindow creates a new main window.
func NewMainWindow(cfg *MainWindowConfig) *MainWindow {
	if cfg.Logger == nil {
		cfg.Logger = slog.Default()
	}

	w := &MainWindow{
		window:         cfg.App.NewWindow("Wardenly"),
		bridge:         cfg.Bridge,
		logger:         cfg.Logger,
		sessionMap:     make(map[string]*SessionTab),
		accountService: cfg.AccountService,
		groupService:   cfg.GroupService,
	}

	// Create CanvasManager (manages CanvasWindow lifecycle and callbacks)
	w.canvasManager = NewCanvasManager(&CanvasManagerConfig{
		App:    cfg.App,
		Bridge: cfg.Bridge,
		Logger: cfg.Logger,
	})

	// Create ScreencastManager (manages screencast lifecycle)
	w.screencastManager = NewScreencastManager(&ScreencastManagerConfig{
		Bridge: cfg.Bridge,
		Logger: cfg.Logger,
	})

	w.init(cfg.ScriptNames)
	w.setupEventCallbacks()
	w.loadAccounts()
	w.loadGroups()

	w.window.SetOnClosed(func() {
		w.Cleanup()
		cfg.App.Quit()
	})

	return w
}

func (w *MainWindow) init(scriptNames []string) {
	w.scriptNames = scriptNames

	toolbar := w.createToolbar()

	// Left sidebar - session list
	w.sessionList = NewSessionList(w.onSessionSelected)
	listWithTitle := container.NewBorder(
		widget.NewLabel("Sessions"),
		nil, nil, nil,
		w.sessionList,
	)

	// Right panel - detail area (initially shows empty message)
	w.emptyDetail = container.NewCenter(widget.NewLabel("Select a session from the list"))
	w.detailPanel = container.NewStack(w.emptyDetail)

	// Left-right split layout
	split := container.NewHSplit(listWithTitle, w.detailPanel)
	split.SetOffset(0.22) // Left side takes ~22%

	content := container.NewBorder(toolbar, nil, nil, nil, split)
	w.window.SetContent(content)
	w.window.Resize(fyne.NewSize(950, 650))
}

func (w *MainWindow) setupEventCallbacks() {
	if w.bridge == nil {
		return
	}

	w.bridge.SetCallbacks(&UICallbacks{
		OnSessionStarted: func(sessionID, accountName string) {
			w.logger.Info("Session started", "session_id", sessionID, "account", accountName)
		},
		OnSessionStopped: func(sessionID string, err error) {
			w.logger.Info("Session stopped", "session_id", sessionID, "error", err)
			// UI update must run on main thread
			fyne.Do(func() {
				w.removeSession(sessionID)
			})
		},
		OnSessionStateChanged: func(sessionID string, oldState, newState state.SessionState) {
			w.logger.Debug("Session state changed", "session_id", sessionID, "from", oldState, "to", newState)
			// UI update must run on main thread
			fyne.Do(func() {
				w.updateSessionState(sessionID, newState)
			})

			// When session becomes Ready, handle initialization that requires session to exist
			if newState == state.StateReady && oldState == state.StateLoggingIn {
				w.onSessionBecameReady(sessionID)
			}
		},
		OnScreenCaptured: func(sessionID string, img image.Image) {
			// Delegate to CanvasManager (handles active session check and UI update)
			if img != nil {
				w.canvasManager.HandleScreenCaptured(sessionID, img)
			}
		},
		OnLoginSucceeded: func(sessionID string) {
			w.logger.Info("Login succeeded", "session_id", sessionID)
			// UI update must run on main thread
			fyne.Do(func() {
				w.enableSessionControls(sessionID)
			})
		},
		OnLoginFailed: func(sessionID string, err error) {
			w.logger.Error("Login failed", "session_id", sessionID, "error", err)
			// UI update must run on main thread
			fyne.Do(func() {
				dialog.ShowError(err, w.window)
				w.enableSessionControls(sessionID) // Enable controls even on failure
			})
		},
		OnScriptStarted: func(sessionID, scriptName string) {
			// UI update must run on main thread
			fyne.Do(func() {
				w.updateScriptState(sessionID, true)
			})
		},
		OnScriptStopped: func(sessionID, scriptName string, reason event.StopReason, err error) {
			// UI update must run on main thread
			fyne.Do(func() {
				w.updateScriptState(sessionID, false)
			})
		},
		OnScreencastStarted: func(sessionID string, quality, maxFPS int) {
			// Delegate to ScreencastManager (must run on UI thread)
			fyne.Do(func() {
				w.screencastManager.OnScreencastStarted(sessionID)
			})
		},
		OnScreencastStopped: func(sessionID string) {
			// Delegate to ScreencastManager (must run on UI thread)
			fyne.Do(func() {
				w.screencastManager.OnScreencastStopped(sessionID)
			})
		},
		OnDriverStarted: func(sessionID string) {
			// Delegate to ScreencastManager (must run on UI thread)
			fyne.Do(func() {
				w.screencastManager.OnDriverStarted(sessionID)
			})
		},
	})
}

func (w *MainWindow) createToolbar() fyne.CanvasObject {
	// Account selection with icon button
	w.accountSelect = widget.NewSelect([]string{}, func(s string) {})
	w.accountSelect.PlaceHolder = "Select Account"
	w.runAccountBtn = widget.NewButtonWithIcon("Run", theme.MediaPlayIcon(), w.handleRunAccount)

	// Group selection with icon button
	w.groupSelect = widget.NewSelect([]string{}, func(s string) {})
	w.groupSelect.PlaceHolder = "Select Group"
	w.runGroupBtn = widget.NewButtonWithIcon("Run", theme.MediaFastForwardIcon(), w.handleRunGroup)

	// Management button with icon
	w.manageBtn = widget.NewButtonWithIcon("Manage...", theme.SettingsIcon(), w.showManagementDialog)

	// Options
	w.spreadToAllCb = widget.NewCheck("Spread to All", func(b bool) {})
	w.autoRefreshCb = widget.NewCheck("Auto Refresh (1s)", func(checked bool) {
		w.screencastManager.SetAutoRefreshEnabled(checked)
	})

	// Layout: Single toolbar row with logical grouping
	// [Account ▼] [▶ Run] | [Group ▼] [▶▶ Run] | spacer | [⚙ Manage...]
	toolbarRow := container.NewHBox(
		w.accountSelect,
		w.runAccountBtn,
		widget.NewSeparator(),
		w.groupSelect,
		w.runGroupBtn,
		layout.NewSpacer(),
		w.manageBtn,
	)

	// Options row (subtle, right-aligned)
	optionsRow := container.NewHBox(
		w.spreadToAllCb,
		w.autoRefreshCb,
	)

	return container.NewVBox(
		toolbarRow,
		optionsRow,
	)
}

func (w *MainWindow) loadAccounts() {
	if w.accountService == nil {
		return
	}

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	accounts, err := w.accountService.ListAccounts(ctx)
	if err != nil {
		w.logger.Error("Failed to load accounts", "error", err)
		return
	}

	w.accounts = accounts

	// Update account select
	options := make([]string, len(accounts))
	for i, acc := range accounts {
		options[i] = acc.Identity()
	}
	w.accountSelect.Options = options
	w.accountSelect.Refresh()
}

func (w *MainWindow) loadGroups() {
	if w.groupService == nil {
		return
	}

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	groups, err := w.groupService.ListAllGroups(ctx)
	if err != nil {
		w.logger.Error("Failed to load groups", "error", err)
		return
	}

	w.groups = groups

	// Update group select options
	options := make([]string, len(groups))
	for i, grp := range groups {
		options[i] = grp.Name
	}

	w.groupSelect.Options = options
	w.groupSelect.Refresh()
}

func (w *MainWindow) handleRunAccount() {
	if w.accountSelect.Selected == "" {
		return
	}

	// Find selected account
	var selectedAcc *account.Account
	for _, acc := range w.accounts {
		if acc.Identity() == w.accountSelect.Selected {
			selectedAcc = acc
			break
		}
	}

	if selectedAcc == nil {
		return
	}

	// Check if already running
	w.sessionMapMu.RLock()
	_, exists := w.sessionMap[selectedAcc.ID]
	w.sessionMapMu.RUnlock()

	if exists {
		dialog.ShowInformation("Account Running",
			"This account is already running.",
			w.window)
		return
	}

	w.runAccount(selectedAcc, true) // Single account run: always select after create
}

func (w *MainWindow) handleRunGroup() {
	if w.groupSelect.Selected == "" {
		return
	}

	// Find selected group
	var selectedGroup *group.Group
	for _, grp := range w.groups {
		if grp.Name == w.groupSelect.Selected {
			selectedGroup = grp
			break
		}
	}

	if selectedGroup == nil {
		return
	}

	// Resolve group accounts (filters out invalid accounts)
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	resolved, err := w.groupService.GetGroupWithAccounts(ctx, selectedGroup.ID)
	cancel()

	if err != nil {
		w.logger.Error("Failed to resolve group accounts", "error", err)
		dialog.ShowError(err, w.window)
		return
	}

	if len(resolved.Accounts) == 0 {
		dialog.ShowInformation("Empty Group",
			"This group has no valid accounts.",
			w.window)
		return
	}

	// Determine if there is already an active session
	hadActiveSession := w.currentSessionID != ""
	firstCreated := false

	// Start accounts serially in background
	go func() {
		for i, acc := range resolved.Accounts {
			// Check if already running
			w.sessionMapMu.RLock()
			_, exists := w.sessionMap[acc.ID]
			w.sessionMapMu.RUnlock()

			if exists {
				continue
			}

			w.logger.Info("Starting account", "account", acc.Identity(), "progress", i+1)

			// Only select if: no active session existed AND this is the first one we create
			shouldSelect := !hadActiveSession && !firstCreated
			w.runAccount(acc, shouldSelect)
			if shouldSelect {
				firstCreated = true
			}

			// Wait between accounts
			if i < len(resolved.Accounts)-1 {
				time.Sleep(3 * time.Second)
			}
		}
	}()
}

func (w *MainWindow) runAccount(acc *account.Account, selectAfterCreate bool) {
	// Create session tab (reusing existing component)
	sessionTab := NewSessionTab(&SessionTabConfig{
		SessionID:   acc.ID,
		AccountName: acc.Identity(),
		Bridge:      w.bridge,
		Logger:      w.logger,
		ScriptNames: w.scriptNames,
		OnStop: func(sessionID string) {
			w.removeSession(sessionID)
		},
		ShouldSpreadToAll: func() bool {
			return w.spreadToAllCb.Checked
		},
		IsAutoRefreshEnabled: func() bool {
			return w.autoRefreshCb.Checked
		},
		OnSyncScript: func(scriptName string) {
			w.syncScriptToAllTabs(scriptName)
		},
		OnStartAllScripts: w.startAllScripts,
		OnStopAllScripts:  w.stopAllScripts,
	})

	// Add to session map
	w.sessionMapMu.Lock()
	w.sessionMap[acc.ID] = sessionTab
	w.sessionMapMu.Unlock()

	// Register with CanvasManager (handles cooldown tracking)
	w.canvasManager.RegisterSession(acc.ID, sessionTab)

	// Add to sidebar list
	w.sessionList.AddSession(acc.ID, acc.Identity())

	// Optionally select the new session
	// Note: SelectSession triggers OnSelected callback which calls onSessionSelected
	if selectAfterCreate {
		w.sessionList.SelectSession(acc.ID)
	}

	// Start session via bridge
	go func() {
		// Convert cookies to command.Cookie
		var cmdCookies []command.Cookie
		if len(acc.Cookies) > 0 {
			cmdCookies = make([]command.Cookie, len(acc.Cookies))
			for i, c := range acc.Cookies {
				cmdCookies[i] = command.Cookie{
					Name:       c.Name,
					Value:      c.Value,
					Domain:     c.Domain,
					Path:       c.Path,
					HTTPOnly:   c.HTTPOnly,
					Secure:     c.Secure,
					SourcePort: c.SourcePort,
				}
			}
		}

		// Pass RoleName (not Identity) to avoid double-prefixing with ServerID
		if err := w.bridge.StartSession(acc.ID, acc.RoleName, acc.UserName, acc.Password, acc.ServerID, cmdCookies); err != nil {
			w.logger.Error("Failed to start session", "error", err)
			dialog.ShowError(err, w.window)
			w.removeSession(acc.ID)
		}
	}()
}

// onSessionSelected handles selection of a session from the sidebar list.
func (w *MainWindow) onSessionSelected(sessionID string) {
	w.sessionMapMu.RLock()
	sessionTab, exists := w.sessionMap[sessionID]
	w.sessionMapMu.RUnlock()

	if !exists {
		return
	}

	w.currentSessionID = sessionID

	// Update detail panel with this session's container
	w.detailPanel.Objects = []fyne.CanvasObject{sessionTab.Container()}
	w.detailPanel.Refresh()

	// Activate canvas for this session (CanvasManager handles callbacks and visibility)
	w.canvasManager.ActivateSession(sessionID)

	// Notify ScreencastManager of session change (handles switching/cancellation internally)
	w.screencastManager.SetActiveSession(sessionID)

	// If not in auto-refresh mode, just capture a single screenshot
	if !w.screencastManager.IsAutoRefreshEnabled() {
		w.canvasManager.RequestCaptureForSession(sessionID, false)
	}
}

func (w *MainWindow) removeSession(sessionID string) {
	// Notify ScreencastManager of session removal
	w.screencastManager.OnSessionRemoved(sessionID)

	// Get the index before removing (for adjacent selection)
	removedIndex := w.sessionList.IndexOf(sessionID)

	// Remove from session map
	w.sessionMapMu.Lock()
	delete(w.sessionMap, sessionID)
	w.sessionMapMu.Unlock()

	// Unregister from CanvasManager (handles canvas state if this was active)
	w.canvasManager.UnregisterSession(sessionID)

	// Remove from sidebar list
	w.sessionList.RemoveSession(sessionID)

	// If this was the current session, switch to adjacent or show empty
	if w.currentSessionID == sessionID {
		w.currentSessionID = ""
		newCount := w.sessionList.Count()

		if newCount > 0 {
			// Prefer next (same index after removal), fallback to previous if removed was last
			var nextID string
			if removedIndex < newCount {
				// There's still an item at this index (the "next" item shifted down)
				nextID = w.sessionList.SessionIDAt(removedIndex)
			} else {
				// Removed was the last item, select previous
				nextID = w.sessionList.SessionIDAt(removedIndex - 1)
			}
			if nextID != "" {
				w.sessionList.SelectSession(nextID)
				w.onSessionSelected(nextID)
			}
		} else {
			// No sessions left, show empty detail
			w.detailPanel.Objects = []fyne.CanvasObject{w.emptyDetail}
			w.detailPanel.Refresh()
			// CanvasManager already handles hiding when last session is unregistered
		}
	}
}

func (w *MainWindow) showManagementDialog() {
	ShowManagementDialog(&ManagementDialogConfig{
		Parent:         w.window,
		AccountService: w.accountService,
		GroupService:   w.groupService,
		Logger:         w.logger,
		OnDataChanged: func() {
			// Reload accounts and groups in main window
			w.loadAccounts()
			w.loadGroups()
		},
	})
}

func (w *MainWindow) syncScriptToAllTabs(scriptName string) {
	if scriptName == "" {
		return
	}

	// 1. Update all Tab UI dropdowns (visual feedback)
	w.sessionMapMu.RLock()
	for _, tab := range w.sessionMap {
		tab.SetScriptSelection(scriptName)
	}
	w.sessionMapMu.RUnlock()

	// 2. Sync to all Session actors via Coordinator (A2: explicit command)
	if err := w.bridge.SyncScriptSelection(scriptName); err != nil {
		w.logger.Error("Failed to sync script selection to sessions", "error", err)
	}

	w.logger.Info("Synced script to all sessions", "script", scriptName)
}

func (w *MainWindow) startAllScripts() {
	// Use bridge to dispatch StartAllScripts command.
	// Coordinator will check each session's CanStartScript() state.
	if err := w.bridge.StartAllScripts(); err != nil {
		w.logger.Error("Failed to start all scripts", "error", err)
	}
}

func (w *MainWindow) stopAllScripts() {
	// Use bridge to dispatch StopAllScripts command.
	// Coordinator will check each session's CanStopScript() state.
	if err := w.bridge.StopAllScripts(); err != nil {
		w.logger.Error("Failed to stop all scripts", "error", err)
	}
}

func (w *MainWindow) updateSessionState(sessionID string, newState state.SessionState) {
	w.sessionMapMu.RLock()
	tab, exists := w.sessionMap[sessionID]
	w.sessionMapMu.RUnlock()

	if !exists {
		return
	}

	tab.UpdateState(newState)
}

func (w *MainWindow) updateScriptState(sessionID string, running bool) {
	w.sessionMapMu.RLock()
	tab, exists := w.sessionMap[sessionID]
	w.sessionMapMu.RUnlock()

	if !exists {
		return
	}

	tab.SetScriptRunning(running)

	// Also update the sidebar list indicator
	w.sessionList.UpdateSessionState(sessionID, running)
}

func (w *MainWindow) enableSessionControls(sessionID string) {
	w.sessionMapMu.RLock()
	tab, exists := w.sessionMap[sessionID]
	w.sessionMapMu.RUnlock()

	if !exists {
		return
	}

	tab.EnableControls()
}

// onSessionBecameReady is called when a session transitions from LoggingIn to Ready.
// This syncs script selection from UI to Session.
func (w *MainWindow) onSessionBecameReady(sessionID string) {
	// Sync script selection from UI to Session
	w.sessionMapMu.RLock()
	tab, exists := w.sessionMap[sessionID]
	w.sessionMapMu.RUnlock()

	if exists && tab.scriptSelect != nil && tab.scriptSelect.Selected != "" {
		if err := w.bridge.SetScriptSelection(sessionID, tab.scriptSelect.Selected); err != nil {
			w.logger.Warn("Failed to sync script selection on ready", "session_id", sessionID, "error", err)
		} else {
			w.logger.Debug("Script selection synced on ready", "session_id", sessionID, "script", tab.scriptSelect.Selected)
		}
	}
	// Note: Screencast is now started via DriverStarted event, not on Ready transition
}

// Public methods

// Show displays the main window.
func (w *MainWindow) Show() {
	w.window.Show()
}

// Cleanup releases resources.
func (w *MainWindow) Cleanup() {
	w.cleanupOnce.Do(func() {
		w.logger.Info("Starting cleanup...")

		// Close ScreencastManager (stops active screencast and pending timers)
		if w.screencastManager != nil {
			w.screencastManager.Close()
		}

		time.Sleep(100 * time.Millisecond)

		if w.bridge != nil {
			w.bridge.StopAllSessions()
		}

		if w.canvasManager != nil {
			w.canvasManager.Close()
		}

		w.sessionMapMu.Lock()
		w.sessionMap = nil
		w.sessionMapMu.Unlock()

		w.logger.Info("Cleanup completed")
	})
}
