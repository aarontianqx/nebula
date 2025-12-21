package presentation

import (
	"context"
	"image"
	"log/slog"
	"sync"
	"sync/atomic"
	"time"

	"fyne.io/fyne/v2"
)

// CanvasManager manages CanvasWindow lifecycle and callbacks with serial command processing.
// All state modifications are serialized through cmdChan to avoid race conditions.
type CanvasManager struct {
	canvasWindow *CanvasWindow

	// State (all access serialized through cmdChan)
	activeSessionID  string
	sessionCallbacks map[string]*CanvasCallbacks
	sessionCreatedAt map[string]time.Time // Cooldown management (migrated from MainWindow)

	// Screenshot throttling (preserves existing mechanism)
	captureInProgress atomic.Bool

	// Frame update throttling (prevents UI freeze from screencast)
	frameUpdatePending atomic.Bool

	// Serial command processing
	cmdChan chan canvasCmd

	// Dependencies
	bridge *UIEventBridge
	logger *slog.Logger

	// Lifecycle
	ctx    context.Context
	cancel context.CancelFunc
	wg     sync.WaitGroup
}

// CanvasCallbacks holds the callbacks for a session's canvas interactions.
type CanvasCallbacks struct {
	sessionTab *SessionTab
	onClick    func(x, y float32)
	onDrag     func(fromX, fromY, toX, toY float32)
}

// canvasCmdType defines the type of canvas command.
type canvasCmdType int

const (
	cmdRegisterSession canvasCmdType = iota
	cmdUnregisterSession
	cmdActivateSession
	cmdDeactivate
	cmdUpdateImage
	cmdRequestCapture
)

// canvasCmd represents a command to be processed by CanvasManager.
type canvasCmd struct {
	typ       canvasCmdType
	sessionID string
	tab       *SessionTab
	image     image.Image
	saveFile  bool
}

// CanvasManagerConfig holds configuration for CanvasManager.
type CanvasManagerConfig struct {
	App    fyne.App
	Bridge *UIEventBridge
	Logger *slog.Logger
}

// NewCanvasManager creates a new CanvasManager.
func NewCanvasManager(cfg *CanvasManagerConfig) *CanvasManager {
	if cfg.Logger == nil {
		cfg.Logger = slog.Default()
	}

	ctx, cancel := context.WithCancel(context.Background())

	m := &CanvasManager{
		canvasWindow:     NewCanvasWindow(cfg.App),
		sessionCallbacks: make(map[string]*CanvasCallbacks),
		sessionCreatedAt: make(map[string]time.Time),
		cmdChan:          make(chan canvasCmd, 100),
		bridge:           cfg.Bridge,
		logger:           cfg.Logger,
		ctx:              ctx,
		cancel:           cancel,
	}

	m.wg.Add(1)
	go m.run()

	return m
}

// run is the main command processing loop.
func (m *CanvasManager) run() {
	defer m.wg.Done()

	for {
		select {
		case <-m.ctx.Done():
			return
		case cmd, ok := <-m.cmdChan:
			if !ok {
				return
			}
			m.processCommand(cmd)
		}
	}
}

// processCommand handles a single command (runs in cmdChan goroutine).
func (m *CanvasManager) processCommand(cmd canvasCmd) {
	defer func() {
		if r := recover(); r != nil {
			m.logger.Error("CanvasManager command panicked", "error", r, "cmd", cmd.typ)
		}
	}()

	switch cmd.typ {
	case cmdRegisterSession:
		m.handleRegisterSession(cmd)
	case cmdUnregisterSession:
		m.handleUnregisterSession(cmd)
	case cmdActivateSession:
		m.handleActivateSession(cmd)
	case cmdDeactivate:
		m.handleDeactivate()
	case cmdUpdateImage:
		m.handleUpdateImage(cmd)
	case cmdRequestCapture:
		m.handleRequestCapture(cmd)
	}
}

// handleRegisterSession registers a session's callbacks.
func (m *CanvasManager) handleRegisterSession(cmd canvasCmd) {
	if cmd.tab == nil {
		return
	}

	// Create callbacks that capture the canvasWindow reference
	callbacks := &CanvasCallbacks{
		sessionTab: cmd.tab,
		onClick:    cmd.tab.HandleCanvasClick(m.canvasWindow),
		onDrag:     cmd.tab.HandleCanvasDrag(m.canvasWindow),
	}

	m.sessionCallbacks[cmd.sessionID] = callbacks
	m.sessionCreatedAt[cmd.sessionID] = time.Now()

	m.logger.Debug("Session registered with CanvasManager", "session_id", cmd.sessionID)
}

// handleUnregisterSession removes a session and handles canvas state.
func (m *CanvasManager) handleUnregisterSession(cmd canvasCmd) {
	delete(m.sessionCallbacks, cmd.sessionID)
	delete(m.sessionCreatedAt, cmd.sessionID)

	m.logger.Debug("Session unregistered from CanvasManager", "session_id", cmd.sessionID, "remaining_count", len(m.sessionCallbacks))

	// If this was the active session, we need to handle it
	if m.activeSessionID == cmd.sessionID {
		m.activeSessionID = ""

		// Clear callbacks to avoid dangling references
		fyne.Do(func() {
			m.canvasWindow.ClearCallbacks()
		})

		// Check if there are other sessions to activate
		if len(m.sessionCallbacks) == 0 {
			// No sessions left, hide canvas
			fyne.Do(func() {
				m.canvasWindow.Hide()
				m.logger.Debug("Canvas window Hide() called due to no sessions")
			})
			m.logger.Debug("No sessions left, hiding canvas")
		}
		// Note: MainWindow will handle selecting the next session and calling ActivateSession
	}
}

// handleActivateSession activates a session's canvas.
func (m *CanvasManager) handleActivateSession(cmd canvasCmd) {
	callbacks, exists := m.sessionCallbacks[cmd.sessionID]
	if !exists {
		m.logger.Warn("Cannot activate unregistered session", "session_id", cmd.sessionID, "registered_count", len(m.sessionCallbacks))
		return
	}

	m.activeSessionID = cmd.sessionID

	// Set callbacks and show canvas on UI thread
	fyne.Do(func() {
		m.canvasWindow.SetOnClicked(callbacks.onClick)
		m.canvasWindow.SetOnDragged(callbacks.onDrag)
		m.canvasWindow.Show()
		m.logger.Debug("Canvas window Show() called", "session_id", cmd.sessionID)
	})

	m.logger.Debug("Session activated", "session_id", cmd.sessionID)
}

// handleDeactivate deactivates the current session.
func (m *CanvasManager) handleDeactivate() {
	m.activeSessionID = ""

	fyne.Do(func() {
		m.canvasWindow.ClearCallbacks()
		m.canvasWindow.Hide()
	})

	m.logger.Debug("Canvas deactivated")
}

// handleUpdateImage updates the canvas image.
// Uses frameUpdatePending to throttle updates and prevent UI freeze from screencast.
func (m *CanvasManager) handleUpdateImage(cmd canvasCmd) {
	// Only update if this is the active session
	if cmd.sessionID != m.activeSessionID {
		return
	}

	if cmd.image == nil {
		return
	}

	// Skip if previous frame update is still pending (throttle)
	if m.frameUpdatePending.Load() {
		return
	}
	m.frameUpdatePending.Store(true)

	fyne.Do(func() {
		m.canvasWindow.SetImage(cmd.image)
		m.frameUpdatePending.Store(false)
	})
}

// handleRequestCapture handles screenshot capture requests with throttling.
func (m *CanvasManager) handleRequestCapture(cmd canvasCmd) {
	// Throttle: skip if previous capture is still in progress
	if m.captureInProgress.Load() {
		return
	}

	// Check if canvas is visible
	if !m.canvasWindow.IsVisible() {
		return
	}

	// Determine target session: use provided sessionID or fallback to active
	targetSessionID := cmd.sessionID
	if targetSessionID == "" {
		targetSessionID = m.activeSessionID
	}

	// Check if we have a valid target session
	if targetSessionID == "" {
		return
	}

	// Check if this is the active session (only capture for active session)
	if targetSessionID != m.activeSessionID {
		return
	}

	// Cooldown: skip auto-refresh screenshot for newly created sessions (first 500ms)
	if createdAt, exists := m.sessionCreatedAt[targetSessionID]; exists {
		if time.Since(createdAt) < 500*time.Millisecond {
			return
		}
	}

	m.captureInProgress.Store(true)

	// Request screenshot via bridge (async)
	go func() {
		defer m.captureInProgress.Store(false)
		defer func() {
			if r := recover(); r != nil {
				m.logger.Error("Screenshot capture panicked", "error", r)
			}
		}()

		if m.bridge != nil {
			if err := m.bridge.CaptureScreen(targetSessionID, cmd.saveFile); err != nil {
				m.logger.Debug("Failed to capture screenshot", "error", err)
			}
		}
	}()
}

// Public methods (send commands to the processing loop)

// RegisterSession registers a session with the canvas manager.
// Called when a new session is created.
func (m *CanvasManager) RegisterSession(sessionID string, tab *SessionTab) {
	select {
	case m.cmdChan <- canvasCmd{typ: cmdRegisterSession, sessionID: sessionID, tab: tab}:
	case <-m.ctx.Done():
	}
}

// UnregisterSession removes a session from the canvas manager.
// Called when a session is stopped/removed.
func (m *CanvasManager) UnregisterSession(sessionID string) {
	select {
	case m.cmdChan <- canvasCmd{typ: cmdUnregisterSession, sessionID: sessionID}:
	case <-m.ctx.Done():
	}
}

// ActivateSession activates a session's canvas.
// Called when a session is selected in the UI.
func (m *CanvasManager) ActivateSession(sessionID string) {
	select {
	case m.cmdChan <- canvasCmd{typ: cmdActivateSession, sessionID: sessionID}:
	case <-m.ctx.Done():
	}
}

// Deactivate deactivates the current canvas.
func (m *CanvasManager) Deactivate() {
	select {
	case m.cmdChan <- canvasCmd{typ: cmdDeactivate}:
	case <-m.ctx.Done():
	}
}

// HandleScreenCaptured handles a screen captured event.
// Called from the event bridge when a screenshot is captured.
func (m *CanvasManager) HandleScreenCaptured(sessionID string, img image.Image) {
	select {
	case m.cmdChan <- canvasCmd{typ: cmdUpdateImage, sessionID: sessionID, image: img}:
	case <-m.ctx.Done():
	}
}

// RequestCapture requests a screenshot capture for the active session.
// Called by auto-refresh or manual capture requests.
// Note: Uses empty sessionID; handleRequestCapture will use the current activeSessionID.
func (m *CanvasManager) RequestCapture(saveFile bool) {
	select {
	case m.cmdChan <- canvasCmd{typ: cmdRequestCapture, sessionID: "", saveFile: saveFile}:
	case <-m.ctx.Done():
	}
}

// RequestCaptureForSession requests a screenshot for a specific session.
func (m *CanvasManager) RequestCaptureForSession(sessionID string, saveFile bool) {
	select {
	case m.cmdChan <- canvasCmd{typ: cmdRequestCapture, sessionID: sessionID, saveFile: saveFile}:
	case <-m.ctx.Done():
	}
}

// IsVisible returns whether the canvas window is visible.
func (m *CanvasManager) IsVisible() bool {
	return m.canvasWindow.IsVisible()
}

// GetImage returns the current canvas image.
func (m *CanvasManager) GetImage() image.Image {
	return m.canvasWindow.GetImage()
}

// Close shuts down the canvas manager.
func (m *CanvasManager) Close() {
	m.cancel()
	close(m.cmdChan)

	// Wait with timeout
	done := make(chan struct{})
	go func() {
		m.wg.Wait()
		close(done)
	}()

	select {
	case <-done:
	case <-time.After(2 * time.Second):
		m.logger.Warn("CanvasManager close timeout")
	}

	// Close the canvas window
	if m.canvasWindow != nil {
		m.canvasWindow.Close()
	}

	m.logger.Info("CanvasManager closed")
}
