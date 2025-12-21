// Package application provides the application layer for orchestrating sessions.
package application

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"wardenly-go/application/session"
	"wardenly-go/core/command"
	"wardenly-go/core/event"
	"wardenly-go/core/eventbus"
	"wardenly-go/domain/account"
	domainscene "wardenly-go/domain/scene"
	domainscript "wardenly-go/domain/script"
	"wardenly-go/infrastructure/browser"
	"wardenly-go/infrastructure/ocr"
)

// Coordinator manages multiple sessions and handles cross-session operations.
type Coordinator struct {
	// Sessions
	sessions   map[string]*session.Session
	sessionsMu sync.RWMutex

	// Dependencies
	eventBus       eventbus.EventBus
	sceneRegistry  *domainscene.Registry
	scriptRegistry *domainscript.Registry
	ocrClient      ocr.Client
	driverFactory  DriverFactory
	logger         *slog.Logger

	// Lifecycle
	ctx    context.Context
	cancel context.CancelFunc
}

// DriverFactory creates browser drivers.
type DriverFactory func() browser.Driver

// CoordinatorConfig holds configuration for the Coordinator.
type CoordinatorConfig struct {
	EventBus       eventbus.EventBus
	SceneRegistry  *domainscene.Registry
	ScriptRegistry *domainscript.Registry
	OCRClient      ocr.Client
	DriverFactory  DriverFactory
	Logger         *slog.Logger
}

// NewCoordinator creates a new session coordinator.
func NewCoordinator(cfg *CoordinatorConfig) *Coordinator {
	if cfg.Logger == nil {
		cfg.Logger = slog.Default()
	}

	ctx, cancel := context.WithCancel(context.Background())

	c := &Coordinator{
		sessions:       make(map[string]*session.Session),
		eventBus:       cfg.EventBus,
		sceneRegistry:  cfg.SceneRegistry,
		scriptRegistry: cfg.ScriptRegistry,
		ocrClient:      cfg.OCRClient,
		driverFactory:  cfg.DriverFactory,
		logger:         cfg.Logger,
		ctx:            ctx,
		cancel:         cancel,
	}

	// Subscribe to events if event bus is available
	if c.eventBus != nil {
		c.eventBus.Subscribe(c.handleEvent)
	}

	return c
}

// Start begins the coordinator.
func (c *Coordinator) Start() {
	c.logger.Info("Coordinator started")
}

// Stop shuts down the coordinator and all sessions.
func (c *Coordinator) Stop() {
	c.cancel()

	c.sessionsMu.Lock()
	sessions := make([]*session.Session, 0, len(c.sessions))
	for _, s := range c.sessions {
		sessions = append(sessions, s)
	}
	c.sessions = make(map[string]*session.Session)
	c.sessionsMu.Unlock()

	// Stop all sessions in parallel
	var wg sync.WaitGroup
	for _, s := range sessions {
		wg.Add(1)
		go func(sess *session.Session) {
			defer wg.Done()
			sess.Stop()
		}(s)
	}

	// Wait with timeout
	done := make(chan struct{})
	go func() {
		wg.Wait()
		close(done)
	}()

	select {
	case <-done:
	case <-time.After(5 * time.Second):
		c.logger.Warn("Coordinator stop timeout, some sessions may not have stopped cleanly")
	}

	c.logger.Info("Coordinator stopped")
}

// Dispatch sends a command to the appropriate handler.
func (c *Coordinator) Dispatch(cmd command.Command) error {
	c.logger.Debug("Dispatching command", "command", cmd.CommandName())

	switch cmd := cmd.(type) {
	// Session lifecycle
	case *command.StartSession:
		return c.handleStartSession(cmd)
	case *command.StopSession:
		return c.handleStopSession(cmd)
	case *command.StopAllSessions:
		return c.handleStopAllSessions(cmd)

	// Multi-session operations
	case *command.ClickAll:
		return c.handleClickAll(cmd)
	case *command.DragAll:
		return c.handleDragAll(cmd)
	case *command.StartAllScripts:
		return c.handleStartAllScripts(cmd)
	case *command.StopAllScripts:
		return c.handleStopAllScripts(cmd)
	case *command.SyncScriptSelection:
		return c.handleSyncScriptSelection(cmd)

	// Session-specific commands
	default:
		if sessionCmd, ok := cmd.(command.SessionCommand); ok {
			return c.routeToSession(sessionCmd)
		}
		return fmt.Errorf("unknown command type: %T", cmd)
	}
}

// CreateSession creates a new session for an account.
func (c *Coordinator) CreateSession(acc *account.Account) (*session.Session, error) {
	c.sessionsMu.Lock()
	defer c.sessionsMu.Unlock()

	// Check if session already exists
	sessionID := acc.ID
	if _, exists := c.sessions[sessionID]; exists {
		return nil, fmt.Errorf("session already exists for account %s", acc.Identity())
	}

	// Create browser driver
	var driver browser.Driver
	if c.driverFactory != nil {
		driver = c.driverFactory()
	} else {
		driver = browser.NewChromeDPDriver(nil)
	}

	// Create session
	sess := session.New(&session.Config{
		ID:             sessionID,
		Account:        acc,
		Driver:         driver,
		EventBus:       c.eventBus,
		SceneRegistry:  c.sceneRegistry,
		ScriptRegistry: c.scriptRegistry,
		OCRClient:      c.ocrClient,
		Logger:         c.logger.With("account", acc.Identity()),
	})

	c.sessions[sessionID] = sess
	sess.Start()

	c.logger.Info("Session created", "session_id", sessionID, "account", acc.Identity())
	return sess, nil
}

// GetSession returns a session by ID.
func (c *Coordinator) GetSession(id string) *session.Session {
	c.sessionsMu.RLock()
	defer c.sessionsMu.RUnlock()
	return c.sessions[id]
}

// GetAllSessions returns all active sessions.
func (c *Coordinator) GetAllSessions() []*session.Session {
	c.sessionsMu.RLock()
	defer c.sessionsMu.RUnlock()

	sessions := make([]*session.Session, 0, len(c.sessions))
	for _, s := range c.sessions {
		sessions = append(sessions, s)
	}
	return sessions
}

// GetActiveSessions returns sessions that can accept operations.
func (c *Coordinator) GetActiveSessions() []*session.Session {
	c.sessionsMu.RLock()
	defer c.sessionsMu.RUnlock()

	sessions := make([]*session.Session, 0)
	for _, s := range c.sessions {
		if s.State().CanAcceptOperations() {
			sessions = append(sessions, s)
		}
	}
	return sessions
}

// SessionCount returns the number of active sessions.
func (c *Coordinator) SessionCount() int {
	c.sessionsMu.RLock()
	defer c.sessionsMu.RUnlock()
	return len(c.sessions)
}

// Command handlers

func (c *Coordinator) handleStartSession(cmd *command.StartSession) error {
	acc := &account.Account{
		ID:       cmd.AccountID,
		RoleName: cmd.RoleName,
		UserName: cmd.UserName,
		Password: cmd.Password,
		ServerID: cmd.ServerID,
	}

	// Convert cookies
	if len(cmd.Cookies) > 0 {
		acc.Cookies = make([]account.Cookie, len(cmd.Cookies))
		for i, c := range cmd.Cookies {
			acc.Cookies[i] = account.Cookie{
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

	sess, err := c.CreateSession(acc)
	if err != nil {
		return err
	}

	// Start browser
	return sess.StartBrowser()
}

func (c *Coordinator) handleStopSession(cmd *command.StopSession) error {
	c.sessionsMu.Lock()
	sess, exists := c.sessions[cmd.SessionID()]
	if exists {
		delete(c.sessions, cmd.SessionID())
	}
	c.sessionsMu.Unlock()

	if !exists {
		return fmt.Errorf("session not found: %s", cmd.SessionID())
	}

	sess.Stop()
	c.logger.Info("Session stopped", "session_id", cmd.SessionID())
	return nil
}

func (c *Coordinator) handleStopAllSessions(cmd *command.StopAllSessions) error {
	c.sessionsMu.Lock()
	sessions := make([]*session.Session, 0, len(c.sessions))
	for _, s := range c.sessions {
		sessions = append(sessions, s)
	}
	c.sessions = make(map[string]*session.Session)
	c.sessionsMu.Unlock()

	for _, s := range sessions {
		s.Stop()
	}

	c.logger.Info("All sessions stopped", "count", len(sessions))
	return nil
}

func (c *Coordinator) handleClickAll(cmd *command.ClickAll) error {
	sessions := c.GetActiveSessions()

	var wg sync.WaitGroup
	for _, sess := range sessions {
		wg.Add(1)
		go func(s *session.Session) {
			defer wg.Done()
			clickCmd := command.NewClick(s.ID(), cmd.X, cmd.Y)
			if err := s.Send(clickCmd); err != nil {
				c.logger.Warn("Failed to send click to session", "session_id", s.ID(), "error", err)
			}
		}(sess)
	}
	wg.Wait()

	return nil
}

func (c *Coordinator) handleDragAll(cmd *command.DragAll) error {
	sessions := c.GetActiveSessions()

	var wg sync.WaitGroup
	for _, sess := range sessions {
		wg.Add(1)
		go func(s *session.Session) {
			defer wg.Done()
			dragCmd := command.NewDrag(s.ID(), cmd.Points)
			if err := s.Send(dragCmd); err != nil {
				c.logger.Warn("Failed to send drag to session", "session_id", s.ID(), "error", err)
			}
		}(sess)
	}
	wg.Wait()

	return nil
}

func (c *Coordinator) handleStartAllScripts(cmd *command.StartAllScripts) error {
	sessions := c.GetActiveSessions()

	for _, sess := range sessions {
		if sess.State().CanStartScript() {
			scriptName := sess.SelectedScript()
			if scriptName == "" {
				continue
			}
			startCmd := command.NewStartScript(sess.ID(), scriptName)
			if err := sess.Send(startCmd); err != nil {
				c.logger.Warn("Failed to start script on session", "session_id", sess.ID(), "error", err)
			}
		}
	}

	return nil
}

func (c *Coordinator) handleStopAllScripts(cmd *command.StopAllScripts) error {
	sessions := c.GetActiveSessions()

	for _, sess := range sessions {
		if sess.State().CanStopScript() {
			stopCmd := command.NewStopScript(sess.ID())
			if err := sess.Send(stopCmd); err != nil {
				c.logger.Warn("Failed to stop script on session", "session_id", sess.ID(), "error", err)
			}
		}
	}

	return nil
}

func (c *Coordinator) handleSyncScriptSelection(cmd *command.SyncScriptSelection) error {
	sessions := c.GetAllSessions()

	for _, sess := range sessions {
		selectCmd := command.NewSetScriptSelection(sess.ID(), cmd.ScriptName)
		if err := sess.Send(selectCmd); err != nil {
			c.logger.Warn("Failed to sync script selection", "session_id", sess.ID(), "error", err)
		}
	}

	return nil
}

func (c *Coordinator) routeToSession(cmd command.SessionCommand) error {
	sess := c.GetSession(cmd.SessionID())
	if sess == nil {
		return fmt.Errorf("session not found: %s", cmd.SessionID())
	}
	return sess.Send(cmd)
}

// handleEvent handles events from the event bus.
func (c *Coordinator) handleEvent(e event.Event) {
	switch evt := e.(type) {
	case *event.SessionStopped:
		c.sessionsMu.Lock()
		delete(c.sessions, evt.SessionID())
		c.sessionsMu.Unlock()
		c.logger.Info("Session removed from coordinator", "session_id", evt.SessionID())
	}
}
