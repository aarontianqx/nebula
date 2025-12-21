package presentation

import (
	"log/slog"
	"time"

	"fyne.io/fyne/v2"
)

// ScreencastManager manages screencast lifecycle for auto-refresh.
// All public methods must be called from the UI thread (via fyne.Do).
// It handles:
// - Delayed screencast start after driver startup (1s delay)
// - Tab switching (stop old, start new)
// - Session removal cleanup
// - Auto-refresh toggle
// - Ack-based streaming state (via ScreencastStarted/ScreencastStopped events)
type ScreencastManager struct {
	bridge *UIEventBridge
	logger *slog.Logger

	// State (all access must be on UI thread)
	autoRefreshEnabled bool
	activeSessionID    string
	streamingSessionID string // acknowledged via ScreencastStarted

	// Delayed start tracking
	driverStartedAt  map[string]time.Time
	pendingTimer     *time.Timer
	pendingSessionID string
	pendingGen       uint64 // monotonic token to invalidate stale timer callbacks
}

// ScreencastManagerConfig holds configuration for ScreencastManager.
type ScreencastManagerConfig struct {
	Bridge *UIEventBridge
	Logger *slog.Logger
}

// NewScreencastManager creates a new ScreencastManager.
func NewScreencastManager(cfg *ScreencastManagerConfig) *ScreencastManager {
	if cfg.Logger == nil {
		cfg.Logger = slog.Default()
	}

	return &ScreencastManager{
		bridge:          cfg.Bridge,
		logger:          cfg.Logger,
		driverStartedAt: make(map[string]time.Time),
	}
}

// SetAutoRefreshEnabled enables or disables auto-refresh mode.
// When enabled, switches to streaming mode for the active session.
// When disabled, stops any active screencast.
// Must be called from UI thread.
func (m *ScreencastManager) SetAutoRefreshEnabled(enabled bool) {
	m.autoRefreshEnabled = enabled

	if enabled {
		m.startAutoRefreshForActiveSession()
	} else {
		m.stopAutoRefresh()
	}
}

// SetActiveSession sets the currently active (selected) session.
// Handles screencast switching if auto-refresh is enabled.
// Must be called from UI thread.
func (m *ScreencastManager) SetActiveSession(sessionID string) {
	m.activeSessionID = sessionID

	// Cancel any pending start (for any session)
	m.cancelPending()

	if !m.autoRefreshEnabled || sessionID == "" {
		return
	}

	// Stop current screencast if streaming a different session
	if m.streamingSessionID != "" && m.streamingSessionID != sessionID {
		m.requestStop(m.streamingSessionID)
	}

	// Only start screencast for the new session if its driver has already started
	// (i.e., we've received DriverStarted event for it)
	// If driver hasn't started yet, OnDriverStarted will schedule the start when it does
	if _, driverReady := m.driverStartedAt[sessionID]; driverReady {
		// If not already streaming this session, start it
		if m.streamingSessionID != sessionID {
			m.requestStart(sessionID)
		}
	}
}

// OnDriverStarted is called when a session's browser driver starts.
// Schedules delayed screencast start if conditions are met.
// Must be called from UI thread.
func (m *ScreencastManager) OnDriverStarted(sessionID string) {
	m.driverStartedAt[sessionID] = time.Now()

	// Only schedule if auto-refresh enabled and this is the active session
	if !m.autoRefreshEnabled || m.activeSessionID != sessionID {
		return
	}

	// Schedule screencast start after 1 second
	m.scheduleStart(sessionID, 1*time.Second)
}

// OnScreencastStarted is called when screencast actually starts (ack from Session).
// Must be called from UI thread.
func (m *ScreencastManager) OnScreencastStarted(sessionID string) {
	m.streamingSessionID = sessionID
	m.logger.Debug("Screencast ack: started", "session_id", sessionID)
}

// OnScreencastStopped is called when screencast actually stops (ack from Session).
// Must be called from UI thread.
func (m *ScreencastManager) OnScreencastStopped(sessionID string) {
	if m.streamingSessionID == sessionID {
		m.streamingSessionID = ""
	}
	m.logger.Debug("Screencast ack: stopped", "session_id", sessionID)
}

// OnSessionRemoved cleans up state when a session is removed.
// Must be called from UI thread.
func (m *ScreencastManager) OnSessionRemoved(sessionID string) {
	// Cancel pending start if it's for this session
	if m.pendingSessionID == sessionID {
		m.cancelPending()
	}

	// Clear streaming state if this session was streaming
	// (Don't call StopScreencast as the session is already stopping)
	if m.streamingSessionID == sessionID {
		m.streamingSessionID = ""
	}

	// Cleanup driver start time
	delete(m.driverStartedAt, sessionID)
}

// Close stops any active screencast and cleans up.
// Must be called from UI thread.
func (m *ScreencastManager) Close() {
	m.cancelPending()

	if m.streamingSessionID != "" {
		if err := m.bridge.StopScreencast(m.streamingSessionID); err != nil {
			m.logger.Warn("Failed to stop screencast on close", "session_id", m.streamingSessionID, "error", err)
		}
		m.streamingSessionID = ""
	}
}

// IsAutoRefreshEnabled returns whether auto-refresh is enabled.
// Must be called from UI thread.
func (m *ScreencastManager) IsAutoRefreshEnabled() bool {
	return m.autoRefreshEnabled
}

// Internal methods (all assume UI thread)

func (m *ScreencastManager) cancelPending() {
	if m.pendingTimer != nil {
		m.pendingTimer.Stop()
		m.pendingTimer = nil
	}
	m.pendingSessionID = ""
	// Increment generation to invalidate any in-flight timer callbacks
	m.pendingGen++
}

func (m *ScreencastManager) scheduleStart(sessionID string, delay time.Duration) {
	// Cancel any existing pending start
	m.cancelPending()

	m.pendingGen++
	gen := m.pendingGen
	m.pendingSessionID = sessionID

	m.logger.Debug("Scheduling screencast start", "session_id", sessionID, "delay", delay)

	m.pendingTimer = time.AfterFunc(delay, func() {
		// Timer fires on a goroutine, so hop back to UI thread
		fyne.Do(func() {
			m.handleTimerFired(sessionID, gen)
		})
	})
}

func (m *ScreencastManager) handleTimerFired(sessionID string, gen uint64) {
	// Validate this timer is still current
	if m.pendingGen != gen || m.pendingSessionID != sessionID {
		return
	}

	// Clear pending state
	m.pendingTimer = nil
	m.pendingSessionID = ""

	// Re-check conditions
	if !m.autoRefreshEnabled || m.activeSessionID != sessionID {
		return
	}

	// Start screencast
	m.requestStart(sessionID)
}

func (m *ScreencastManager) requestStart(sessionID string) {
	if err := m.bridge.StartScreencast(sessionID, 80, 5); err != nil {
		m.logger.Error("Failed to start screencast", "session_id", sessionID, "error", err)
	} else {
		m.logger.Info("Screencast started", "session_id", sessionID)
	}
}

func (m *ScreencastManager) requestStop(sessionID string) {
	if err := m.bridge.StopScreencast(sessionID); err != nil {
		m.logger.Warn("Failed to stop screencast", "session_id", sessionID, "error", err)
	} else {
		m.logger.Debug("Screencast stop requested", "session_id", sessionID)
	}
}

func (m *ScreencastManager) startAutoRefreshForActiveSession() {
	sessionID := m.activeSessionID
	if sessionID == "" {
		return
	}

	// If already streaming this session, nothing to do
	if m.streamingSessionID == sessionID {
		return
	}

	// Stop any existing screencast for a different session
	if m.streamingSessionID != "" {
		m.requestStop(m.streamingSessionID)
	}

	// Only start if driver has started for this session
	if _, driverReady := m.driverStartedAt[sessionID]; driverReady {
		m.requestStart(sessionID)
	}
	// If driver hasn't started, OnDriverStarted will handle it when it does
}

func (m *ScreencastManager) stopAutoRefresh() {
	// Cancel any pending start
	m.cancelPending()

	// Stop current screencast if any
	if m.streamingSessionID != "" {
		m.requestStop(m.streamingSessionID)
		// streamingSessionID will be cleared by OnScreencastStopped callback
	}
}
