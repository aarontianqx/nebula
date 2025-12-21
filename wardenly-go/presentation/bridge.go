// Package presentation provides the UI layer with event bridging to the application layer.
package presentation

import (
	"image"
	"log/slog"
	"sync"

	"wardenly-go/application"
	"wardenly-go/core/command"
	"wardenly-go/core/event"
	"wardenly-go/core/eventbus"
	"wardenly-go/core/state"
)

// UIEventBridge bridges UI events to the application layer and routes events back to UI.
// It provides a clean separation between UI and business logic.
type UIEventBridge struct {
	coordinator *application.Coordinator
	eventBus    eventbus.EventBus
	logger      *slog.Logger

	// UI callbacks - set by UI components
	callbacks   *UICallbacks
	callbacksMu sync.RWMutex

	// Subscription management
	subscriptionID string
}

// UICallbacks contains callbacks for UI updates.
type UICallbacks struct {
	// Session lifecycle
	OnSessionStarted      func(sessionID, accountName string)
	OnSessionStopped      func(sessionID string, err error)
	OnSessionStateChanged func(sessionID string, oldState, newState state.SessionState)

	// Browser events
	OnScreenCaptured    func(sessionID string, img image.Image)
	OnLoginSucceeded    func(sessionID string)
	OnLoginFailed       func(sessionID string, err error)
	OnCookiesSaved      func(sessionID string)
	OnOperationFailed   func(sessionID, operation string, err error)
	OnScreencastStarted func(sessionID string, quality, maxFPS int)
	OnScreencastStopped func(sessionID string)
	OnDriverStarted     func(sessionID string)

	// Script events
	OnScriptStarted          func(sessionID, scriptName string)
	OnScriptStopped          func(sessionID, scriptName string, reason event.StopReason, err error)
	OnScriptSelectionChanged func(sessionID, scriptName string)
}

// BridgeConfig holds configuration for UIEventBridge.
type BridgeConfig struct {
	Coordinator *application.Coordinator
	EventBus    eventbus.EventBus
	Logger      *slog.Logger
}

// NewUIEventBridge creates a new UI event bridge.
func NewUIEventBridge(cfg *BridgeConfig) *UIEventBridge {
	if cfg.Logger == nil {
		cfg.Logger = slog.Default()
	}

	b := &UIEventBridge{
		coordinator: cfg.Coordinator,
		eventBus:    cfg.EventBus,
		logger:      cfg.Logger,
		callbacks:   &UICallbacks{},
	}

	// Subscribe to events
	if b.eventBus != nil {
		b.subscriptionID = b.eventBus.Subscribe(b.handleEvent)
	}

	return b
}

// SetCallbacks sets the UI callbacks.
func (b *UIEventBridge) SetCallbacks(callbacks *UICallbacks) {
	b.callbacksMu.Lock()
	defer b.callbacksMu.Unlock()
	b.callbacks = callbacks
}

// Close unsubscribes from the event bus.
func (b *UIEventBridge) Close() {
	if b.eventBus != nil && b.subscriptionID != "" {
		b.eventBus.Unsubscribe(b.subscriptionID)
	}
}

// Command dispatching methods

// StartSession starts a new session for an account.
func (b *UIEventBridge) StartSession(accountID, roleName, userName, password string, serverID int, cookies []command.Cookie) error {
	cmd := &command.StartSession{
		AccountID: accountID,
		RoleName:  roleName,
		UserName:  userName,
		Password:  password,
		ServerID:  serverID,
		Cookies:   cookies,
	}
	return b.coordinator.Dispatch(cmd)
}

// StopSession stops a running session.
func (b *UIEventBridge) StopSession(sessionID string) error {
	return b.coordinator.Dispatch(command.NewStopSession(sessionID))
}

// StopAllSessions stops all running sessions.
func (b *UIEventBridge) StopAllSessions() error {
	return b.coordinator.Dispatch(&command.StopAllSessions{})
}

// Click performs a click at the specified coordinates.
func (b *UIEventBridge) Click(sessionID string, x, y float64) error {
	return b.coordinator.Dispatch(command.NewClick(sessionID, x, y))
}

// ClickAll performs a click on all active sessions.
func (b *UIEventBridge) ClickAll(x, y float64) error {
	return b.coordinator.Dispatch(&command.ClickAll{X: x, Y: y})
}

// Drag performs a drag operation.
func (b *UIEventBridge) Drag(sessionID string, fromX, fromY, toX, toY float64) error {
	return b.coordinator.Dispatch(command.NewDragFromTo(sessionID, fromX, fromY, toX, toY))
}

// DragAll performs a drag on all active sessions.
func (b *UIEventBridge) DragAll(fromX, fromY, toX, toY float64) error {
	return b.coordinator.Dispatch(&command.DragAll{
		Points: []command.Point{{X: fromX, Y: fromY}, {X: toX, Y: toY}},
	})
}

// CaptureScreen captures the current browser screen.
func (b *UIEventBridge) CaptureScreen(sessionID string, saveToFile bool) error {
	return b.coordinator.Dispatch(command.NewCaptureScreen(sessionID, saveToFile))
}

// RefreshPage refreshes the browser page.
func (b *UIEventBridge) RefreshPage(sessionID string) error {
	return b.coordinator.Dispatch(command.NewRefreshPage(sessionID))
}

// SaveCookies saves the current session cookies.
func (b *UIEventBridge) SaveCookies(sessionID string) error {
	return b.coordinator.Dispatch(command.NewSaveCookies(sessionID))
}

// StartScreencast starts frame streaming for a session.
func (b *UIEventBridge) StartScreencast(sessionID string, quality, maxFPS int) error {
	return b.coordinator.Dispatch(command.NewStartScreencast(sessionID, quality, maxFPS))
}

// StopScreencast stops frame streaming for a session.
func (b *UIEventBridge) StopScreencast(sessionID string) error {
	return b.coordinator.Dispatch(command.NewStopScreencast(sessionID))
}

// StartScript starts a script on a session.
func (b *UIEventBridge) StartScript(sessionID, scriptName string) error {
	return b.coordinator.Dispatch(command.NewStartScript(sessionID, scriptName))
}

// StopScript stops the running script on a session.
func (b *UIEventBridge) StopScript(sessionID string) error {
	return b.coordinator.Dispatch(command.NewStopScript(sessionID))
}

// StartAllScripts starts scripts on all sessions.
func (b *UIEventBridge) StartAllScripts() error {
	return b.coordinator.Dispatch(&command.StartAllScripts{})
}

// StopAllScripts stops scripts on all sessions.
func (b *UIEventBridge) StopAllScripts() error {
	return b.coordinator.Dispatch(&command.StopAllScripts{})
}

// SetScriptSelection sets the selected script for a session.
func (b *UIEventBridge) SetScriptSelection(sessionID, scriptName string) error {
	return b.coordinator.Dispatch(command.NewSetScriptSelection(sessionID, scriptName))
}

// SyncScriptSelection synchronizes script selection to all sessions.
func (b *UIEventBridge) SyncScriptSelection(scriptName string) error {
	return b.coordinator.Dispatch(&command.SyncScriptSelection{ScriptName: scriptName})
}

// Query methods

// GetSessionState returns the state of a session.
func (b *UIEventBridge) GetSessionState(sessionID string) state.SessionState {
	sess := b.coordinator.GetSession(sessionID)
	if sess == nil {
		return state.StateIdle
	}
	return sess.State()
}

// GetSessionCount returns the number of active sessions.
func (b *UIEventBridge) GetSessionCount() int {
	return b.coordinator.SessionCount()
}

// IsSessionRunning checks if a session exists and is running.
func (b *UIEventBridge) IsSessionRunning(sessionID string) bool {
	sess := b.coordinator.GetSession(sessionID)
	return sess != nil && sess.State().IsActive()
}

// IsScriptRunning checks if a script is running on a session.
func (b *UIEventBridge) IsScriptRunning(sessionID string) bool {
	sess := b.coordinator.GetSession(sessionID)
	return sess != nil && sess.IsScriptRunning()
}

// Event handling

func (b *UIEventBridge) handleEvent(e event.Event) {
	b.callbacksMu.RLock()
	callbacks := b.callbacks
	b.callbacksMu.RUnlock()

	if callbacks == nil {
		return
	}

	switch evt := e.(type) {
	case *event.SessionStarted:
		if callbacks.OnSessionStarted != nil {
			callbacks.OnSessionStarted(evt.SessionID(), evt.AccountName)
		}

	case *event.SessionStopped:
		if callbacks.OnSessionStopped != nil {
			callbacks.OnSessionStopped(evt.SessionID(), evt.Error)
		}

	case *event.SessionStateChanged:
		if callbacks.OnSessionStateChanged != nil {
			callbacks.OnSessionStateChanged(evt.SessionID(), evt.OldState, evt.NewState)
		}

	case *event.ScreenCaptured:
		if callbacks.OnScreenCaptured != nil {
			callbacks.OnScreenCaptured(evt.SessionID(), evt.Image)
		}

	case *event.LoginSucceeded:
		if callbacks.OnLoginSucceeded != nil {
			callbacks.OnLoginSucceeded(evt.SessionID())
		}

	case *event.LoginFailed:
		if callbacks.OnLoginFailed != nil {
			callbacks.OnLoginFailed(evt.SessionID(), evt.Error)
		}

	case *event.CookiesSaved:
		if callbacks.OnCookiesSaved != nil {
			callbacks.OnCookiesSaved(evt.SessionID())
		}

	case *event.OperationFailed:
		if callbacks.OnOperationFailed != nil {
			callbacks.OnOperationFailed(evt.SessionID(), evt.Operation, evt.Error)
		}

	case *event.ScriptStarted:
		if callbacks.OnScriptStarted != nil {
			callbacks.OnScriptStarted(evt.SessionID(), evt.ScriptName)
		}

	case *event.ScriptStopped:
		if callbacks.OnScriptStopped != nil {
			callbacks.OnScriptStopped(evt.SessionID(), evt.ScriptName, evt.Reason, evt.Error)
		}

	case *event.ScriptSelectionChanged:
		if callbacks.OnScriptSelectionChanged != nil {
			callbacks.OnScriptSelectionChanged(evt.SessionID(), evt.ScriptName)
		}

	case *event.ScreencastStarted:
		if callbacks.OnScreencastStarted != nil {
			callbacks.OnScreencastStarted(evt.SessionID(), evt.Quality, evt.MaxFPS)
		}

	case *event.ScreencastStopped:
		if callbacks.OnScreencastStopped != nil {
			callbacks.OnScreencastStopped(evt.SessionID())
		}

	case *event.DriverStarted:
		if callbacks.OnDriverStarted != nil {
			callbacks.OnDriverStarted(evt.SessionID())
		}
	}
}
