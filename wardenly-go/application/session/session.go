// Package session implements the Session Actor pattern for managing browser sessions.
package session

import (
	"context"
	"fmt"
	"image"
	"log/slog"
	"sync"
	"time"

	"wardenly-go/core/command"
	"wardenly-go/core/event"
	"wardenly-go/core/eventbus"
	"wardenly-go/core/state"
	"wardenly-go/domain/account"
	domainscene "wardenly-go/domain/scene"
	domainscript "wardenly-go/domain/script"
	"wardenly-go/infrastructure/browser"
	"wardenly-go/infrastructure/ocr"
)

// Session represents a single browser session as an Actor.
// It processes commands serially through a command queue, ensuring thread-safe state management.
type Session struct {
	// Identity
	id        string
	accountID string
	account   *account.Account

	// State
	state          state.SessionState
	selectedScript string
	stateMu        sync.RWMutex

	// Components
	browserCtrl  *BrowserController
	scriptRunner *ScriptRunner
	screenCap    *ScreenCapture

	// Dependencies
	driver         browser.Driver
	eventBus       eventbus.EventBus
	sceneRegistry  *domainscene.Registry
	sceneMatcher   *domainscene.Matcher
	scriptRegistry *domainscript.Registry
	ocrClient      ocr.Client
	logger         *slog.Logger

	// Command processing
	cmdChan chan command.Command
	ctx     context.Context
	cancel  context.CancelFunc
	wg      sync.WaitGroup

	// Screencast state
	screencastActive bool
	screencastCancel context.CancelFunc
}

// Config holds configuration for creating a new Session.
type Config struct {
	ID             string
	Account        *account.Account
	Driver         browser.Driver
	EventBus       eventbus.EventBus
	SceneRegistry  *domainscene.Registry
	ScriptRegistry *domainscript.Registry
	OCRClient      ocr.Client
	Logger         *slog.Logger
	CommandBuffer  int
}

// New creates a new Session actor.
func New(cfg *Config) *Session {
	if cfg.Logger == nil {
		cfg.Logger = slog.Default()
	}
	if cfg.CommandBuffer <= 0 {
		cfg.CommandBuffer = 100
	}

	ctx, cancel := context.WithCancel(context.Background())

	s := &Session{
		id:             cfg.ID,
		accountID:      cfg.Account.ID,
		account:        cfg.Account,
		state:          state.StateIdle,
		driver:         cfg.Driver,
		eventBus:       cfg.EventBus,
		sceneRegistry:  cfg.SceneRegistry,
		sceneMatcher:   domainscene.NewMatcher(5.0),
		scriptRegistry: cfg.ScriptRegistry,
		ocrClient:      cfg.OCRClient,
		logger:         cfg.Logger.With("session_id", cfg.ID),
		cmdChan:        make(chan command.Command, cfg.CommandBuffer),
		ctx:            ctx,
		cancel:         cancel,
	}

	// Initialize components
	s.browserCtrl = NewBrowserController(s.driver, s.logger)
	s.screenCap = NewScreenCapture(s.driver, s.logger)
	s.scriptRunner = NewScriptRunner(s, s.logger)

	return s
}

// Start begins the session's command processing loop.
func (s *Session) Start() {
	s.wg.Add(1)
	go s.run()
	s.logger.Info("Session started")
}

// Stop signals the session to stop and waits for cleanup with timeout.
func (s *Session) Stop() {
	s.cancel()
	close(s.cmdChan)

	done := make(chan struct{})
	go func() {
		s.wg.Wait()
		close(done)
	}()

	select {
	case <-done:
		s.logger.Info("Session stopped")
	case <-time.After(3 * time.Second):
		s.logger.Warn("Session stop timeout")
	}
}

// Send sends a command to the session for processing.
// Returns an error if the session is not accepting commands.
func (s *Session) Send(cmd command.Command) error {
	select {
	case s.cmdChan <- cmd:
		return nil
	case <-s.ctx.Done():
		return fmt.Errorf("session is stopped")
	default:
		return fmt.Errorf("command queue full")
	}
}

// ID returns the session ID.
func (s *Session) ID() string {
	return s.id
}

// AccountID returns the associated account ID.
func (s *Session) AccountID() string {
	return s.accountID
}

// Account returns the associated account.
func (s *Session) Account() *account.Account {
	return s.account
}

// State returns the current session state.
func (s *Session) State() state.SessionState {
	s.stateMu.RLock()
	defer s.stateMu.RUnlock()
	return s.state
}

// SelectedScript returns the currently selected script name.
func (s *Session) SelectedScript() string {
	s.stateMu.RLock()
	defer s.stateMu.RUnlock()
	return s.selectedScript
}

// IsScriptRunning returns true if a script is currently running.
func (s *Session) IsScriptRunning() bool {
	return s.State() == state.StateScriptRunning
}

// run is the main command processing loop.
func (s *Session) run() {
	defer s.wg.Done()
	defer s.cleanup()

	for {
		select {
		case <-s.ctx.Done():
			return
		case cmd, ok := <-s.cmdChan:
			if !ok {
				return
			}
			s.processCommand(cmd)
		}
	}
}

// cleanup performs cleanup when the session stops.
func (s *Session) cleanup() {
	// Stop screencast if active
	if s.screencastActive {
		s.stopScreencast()
	}

	// Stop script if running
	if s.IsScriptRunning() {
		s.scriptRunner.Stop()
	}

	// Stop browser
	if s.driver != nil && s.driver.IsRunning() {
		if err := s.driver.Stop(); err != nil {
			s.logger.Error("Failed to stop browser", "error", err)
		}
	}

	s.transitionTo(state.StateStopped)
}

// processCommand handles a single command.
func (s *Session) processCommand(cmd command.Command) {
	s.logger.Debug("Processing command", "command", cmd.CommandName())

	switch c := cmd.(type) {
	// Browser operations
	case *command.Click:
		s.handleClick(c)
	case *command.Drag:
		s.handleDrag(c)
	case *command.CaptureScreen:
		s.handleCaptureScreen(c)
	case *command.RefreshPage:
		s.handleRefreshPage(c)
	case *command.SaveCookies:
		s.handleSaveCookies(c)

	// Screencast operations
	case *command.StartScreencast:
		s.handleStartScreencast(c)
	case *command.StopScreencast:
		s.handleStopScreencast(c)

	// Script operations
	case *command.StartScript:
		s.handleStartScript(c)
	case *command.StopScript:
		s.handleStopScript(c)
	case *command.SetScriptSelection:
		s.handleSetScriptSelection(c)

	// Session lifecycle
	case *command.StopSession:
		s.handleStopSession(c)

	default:
		s.logger.Warn("Unknown command", "command", fmt.Sprintf("%T", cmd))
	}
}

// State transition helpers

func (s *Session) transitionTo(newState state.SessionState) error {
	s.stateMu.Lock()
	oldState := s.state

	if !oldState.CanTransitionTo(newState) {
		s.stateMu.Unlock()
		return state.NewTransitionError(oldState, newState, "invalid transition")
	}

	s.state = newState
	s.stateMu.Unlock()

	// Publish state change event
	s.publishEvent(event.NewSessionStateChanged(s.id, oldState, newState))
	s.logger.Info("State changed", "from", oldState, "to", newState)

	return nil
}

func (s *Session) publishEvent(e event.Event) {
	if s.eventBus != nil {
		s.eventBus.Publish(e)
	}
}

// Command handlers

func (s *Session) handleClick(cmd *command.Click) {
	if !s.State().CanAcceptOperations() {
		s.logger.Warn("Cannot accept click in current state", "state", s.State())
		return
	}

	if err := s.browserCtrl.Click(s.ctx, cmd.X, cmd.Y); err != nil {
		s.logger.Error("Click failed", "error", err)
		s.publishEvent(event.NewOperationFailed(s.id, "click", err))
	}
}

func (s *Session) handleDrag(cmd *command.Drag) {
	if !s.State().CanAcceptOperations() {
		s.logger.Warn("Cannot accept drag in current state", "state", s.State())
		return
	}

	var err error
	if len(cmd.Points) == 2 {
		// Simple two-point drag: use Drag() for smooth 10-step interpolation
		s.logger.Info("Drag",
			"from", fmt.Sprintf("(%.1f, %.1f)", cmd.Points[0].X, cmd.Points[0].Y),
			"to", fmt.Sprintf("(%.1f, %.1f)", cmd.Points[1].X, cmd.Points[1].Y))
		err = s.browserCtrl.Drag(s.ctx,
			cmd.Points[0].X, cmd.Points[0].Y,
			cmd.Points[1].X, cmd.Points[1].Y)
	} else {
		// Multi-point path: use DragPath() with frame-based timing
		points := make([]browser.Point, len(cmd.Points))
		pathStr := make([]string, len(cmd.Points))
		for i, p := range cmd.Points {
			points[i] = browser.Point{X: p.X, Y: p.Y}
			pathStr[i] = fmt.Sprintf("(%.1f, %.1f)", p.X, p.Y)
		}
		s.logger.Info("DragPath", "points", pathStr)
		err = s.browserCtrl.DragPath(s.ctx, points)
	}

	if err != nil {
		s.logger.Error("Drag failed", "error", err)
		s.publishEvent(event.NewOperationFailed(s.id, "drag", err))
	}
}

func (s *Session) handleCaptureScreen(cmd *command.CaptureScreen) {
	if !s.State().CanAcceptOperations() {
		s.logger.Warn("Cannot capture screen in current state", "state", s.State())
		return
	}

	img, err := s.screenCap.Capture(s.ctx)
	if err != nil {
		s.logger.Error("Screen capture failed", "error", err)
		s.publishEvent(event.NewOperationFailed(s.id, "capture_screen", err))
		return
	}

	if cmd.SaveToFile {
		if err := s.screenCap.SaveToFile(img); err != nil {
			s.logger.Error("Failed to save screenshot", "error", err)
		}
	}

	s.publishEvent(event.NewScreenCaptured(s.id, img))
}

func (s *Session) handleRefreshPage(cmd *command.RefreshPage) {
	if !s.State().CanAcceptOperations() {
		s.logger.Warn("Cannot refresh in current state", "state", s.State())
		return
	}

	if err := s.browserCtrl.Refresh(s.ctx); err != nil {
		s.logger.Error("Refresh failed", "error", err)
		s.publishEvent(event.NewOperationFailed(s.id, "refresh", err))
	}
}

func (s *Session) handleSaveCookies(cmd *command.SaveCookies) {
	if !s.State().CanAcceptOperations() {
		s.logger.Warn("Cannot save cookies in current state", "state", s.State())
		return
	}

	cookies, err := s.browserCtrl.GetCookies(s.ctx)
	if err != nil {
		s.logger.Error("Failed to get cookies", "error", err)
		s.publishEvent(event.NewOperationFailed(s.id, "save_cookies", err))
		return
	}

	// Convert to domain cookies
	domainCookies := make([]account.Cookie, len(cookies))
	for i, c := range cookies {
		domainCookies[i] = account.Cookie{
			Name:         c.Name,
			Value:        c.Value,
			Domain:       c.Domain,
			Path:         c.Path,
			HTTPOnly:     c.HTTPOnly,
			Secure:       c.Secure,
			SourcePort:   c.SourcePort,
			SourceScheme: c.SourceScheme,
			Priority:     c.Priority,
		}
	}

	s.account.Cookies = domainCookies
	s.publishEvent(event.NewCookiesSaved(s.id))
	s.logger.Info("Cookies captured", "count", len(cookies))
}

func (s *Session) handleStartScreencast(cmd *command.StartScreencast) {
	// Allow screencast in LoggingIn, Ready, or ScriptRunning states
	// LoggingIn is allowed so users can see login progress/failures
	currentState := s.State()
	if currentState != state.StateLoggingIn && currentState != state.StateReady && currentState != state.StateScriptRunning {
		s.logger.Debug("Screencast not allowed in current state", "state", currentState)
		return
	}

	if s.screencastActive {
		s.logger.Debug("Screencast already active")
		return
	}

	// Start screencast on driver
	frameChan, err := s.driver.StartScreencast(s.ctx, cmd.Quality, cmd.MaxFPS)
	if err != nil {
		s.logger.Error("Failed to start screencast", "error", err)
		s.publishEvent(event.NewOperationFailed(s.id, "start_screencast", err))
		return
	}

	s.screencastActive = true

	// Create cancellation context for the frame forwarding goroutine
	screencastCtx, screencastCancel := context.WithCancel(s.ctx)
	s.screencastCancel = screencastCancel

	// Start goroutine to forward frames as events
	s.wg.Add(1)
	go s.forwardScreencastFrames(screencastCtx, frameChan)

	// Publish event so UI knows screencast actually started
	s.publishEvent(event.NewScreencastStarted(s.id, cmd.Quality, cmd.MaxFPS))
	s.logger.Info("Screencast started", "quality", cmd.Quality, "maxFPS", cmd.MaxFPS)
}

func (s *Session) handleStopScreencast(cmd *command.StopScreencast) {
	s.stopScreencast()
}

// stopScreencast stops the active screencast.
func (s *Session) stopScreencast() {
	if !s.screencastActive {
		return
	}

	// Cancel the frame forwarding goroutine
	if s.screencastCancel != nil {
		s.screencastCancel()
		s.screencastCancel = nil
	}

	// Stop screencast on driver
	if err := s.driver.StopScreencast(); err != nil {
		s.logger.Error("Failed to stop screencast", "error", err)
	}

	s.screencastActive = false

	// Publish event so UI knows screencast stopped
	s.publishEvent(event.NewScreencastStopped(s.id))
	s.logger.Info("Screencast stopped")
}

// forwardScreencastFrames forwards frames from the driver channel to events.
func (s *Session) forwardScreencastFrames(ctx context.Context, frameChan <-chan image.Image) {
	defer s.wg.Done()

	for {
		select {
		case <-ctx.Done():
			return
		case img, ok := <-frameChan:
			if !ok {
				// Channel closed, screencast ended
				return
			}
			s.publishEvent(event.NewScreenCaptured(s.id, img))
		}
	}
}

// IsScreencasting returns true if screencast is active.
func (s *Session) IsScreencasting() bool {
	return s.screencastActive
}

func (s *Session) handleStartScript(cmd *command.StartScript) {
	if !s.State().CanStartScript() {
		s.logger.Warn("Cannot start script in current state", "state", s.State())
		return
	}

	script := s.scriptRegistry.Get(cmd.ScriptName)
	if script == nil {
		s.logger.Error("Script not found", "name", cmd.ScriptName)
		return
	}

	if err := s.transitionTo(state.StateScriptRunning); err != nil {
		s.logger.Error("Failed to transition to script running state", "error", err)
		return
	}

	s.scriptRunner.Start(script)
	s.publishEvent(event.NewScriptStarted(s.id, cmd.ScriptName))
}

func (s *Session) handleStopScript(cmd *command.StopScript) {
	if !s.State().CanStopScript() {
		s.logger.Warn("Cannot stop script in current state", "state", s.State())
		return
	}

	s.scriptRunner.Stop()
}

func (s *Session) handleSetScriptSelection(cmd *command.SetScriptSelection) {
	s.stateMu.Lock()
	s.selectedScript = cmd.ScriptName
	s.stateMu.Unlock()

	s.publishEvent(event.NewScriptSelectionChanged(s.id, cmd.ScriptName))
}

func (s *Session) handleStopSession(cmd *command.StopSession) {
	s.logger.Info("Stop session requested")
	s.cancel()
}

// Methods called by ScriptRunner

// OnScriptStopped is called when the script runner finishes.
func (s *Session) OnScriptStopped(scriptName string, reason event.StopReason, err error) {
	if s.State() == state.StateScriptRunning {
		if transErr := s.transitionTo(state.StateReady); transErr != nil {
			s.logger.Error("Failed to transition from script running", "error", transErr)
		}
	}
	s.publishEvent(event.NewScriptStopped(s.id, scriptName, reason, err))
}

// GetScreenCapture returns the screen capture component.
func (s *Session) GetScreenCapture() *ScreenCapture {
	return s.screenCap
}

// GetBrowserController returns the browser controller.
func (s *Session) GetBrowserController() *BrowserController {
	return s.browserCtrl
}

// GetSceneRegistry returns the scene registry.
func (s *Session) GetSceneRegistry() *domainscene.Registry {
	return s.sceneRegistry
}

// GetSceneMatcher returns the scene matcher.
func (s *Session) GetSceneMatcher() *domainscene.Matcher {
	return s.sceneMatcher
}

// GetOCRClient returns the OCR client.
func (s *Session) GetOCRClient() ocr.Client {
	return s.ocrClient
}

// Context returns the session's context.
func (s *Session) Context() context.Context {
	return s.ctx
}

// StartBrowser initializes and starts the browser for login.
func (s *Session) StartBrowser() error {
	if err := s.transitionTo(state.StateStarting); err != nil {
		return err
	}

	if err := s.driver.Start(s.ctx); err != nil {
		s.transitionTo(state.StateStopped)
		return fmt.Errorf("failed to start browser: %w", err)
	}

	if err := s.transitionTo(state.StateLoggingIn); err != nil {
		return err
	}

	// Notify that browser driver is started and ready to render frames
	s.publishEvent(event.NewDriverStarted(s.id))

	// Perform login (this would be async in real implementation)
	go s.performLogin()

	return nil
}

func (s *Session) performLogin() {
	// URL format for the game
	url := fmt.Sprintf("http://www.lequ.com/server/wly/s/%d", s.account.ServerID)

	var loginErr error
	if len(s.account.Cookies) > 0 {
		// Login with cookies
		s.logger.Info("Cookies not empty, try to login by cookies")
		loginErr = s.loginWithCookies(url)
	} else {
		// Login with username/password
		s.logger.Info("Cookies empty, try to login by user password")
		loginErr = s.loginWithUserPassword(url)
	}

	if loginErr != nil {
		s.logger.Error("Login failed", "error", loginErr)
		s.publishEvent(event.NewLoginFailed(s.id, loginErr))
		// Even on login failure, transition to Ready state so user can manually operate
		if err := s.transitionTo(state.StateReady); err != nil {
			s.logger.Error("Failed to transition to ready after login failure", "error", err)
		}
		return
	}

	// Wait for game to fully load
	if err := s.waitLoadingGame(); err != nil {
		s.logger.Error("Wait loading game failed", "error", err)
		s.publishEvent(event.NewLoginFailed(s.id, err))
		// Even on wait failure, transition to Ready state so user can manually operate
		if err := s.transitionTo(state.StateReady); err != nil {
			s.logger.Error("Failed to transition to ready after wait failure", "error", err)
		}
		return
	}

	// Login successful - save cookies
	if err := s.saveCookiesAfterLogin(); err != nil {
		s.logger.Warn("Failed to save cookies after login", "error", err)
	} else {
		s.logger.Info("Cookies saved")
	}

	// Transition to ready state
	if err := s.transitionTo(state.StateReady); err != nil {
		s.logger.Error("Failed to transition to ready", "error", err)
		return
	}
	s.publishEvent(event.NewLoginSucceeded(s.id))
	s.logger.Info("Login successful")
}

// loginWithCookies attempts to login using stored cookies.
func (s *Session) loginWithCookies(url string) error {
	// Convert domain cookies to browser cookies
	browserCookies := make([]browser.Cookie, len(s.account.Cookies))
	for i, c := range s.account.Cookies {
		browserCookies[i] = browser.Cookie{
			Name:         c.Name,
			Value:        c.Value,
			Domain:       c.Domain,
			Path:         c.Path,
			HTTPOnly:     c.HTTPOnly,
			Secure:       c.Secure,
			SourcePort:   c.SourcePort,
			SourceScheme: c.SourceScheme,
			Priority:     c.Priority,
		}
	}

	// Use the driver's LoginWithCookies method which executes all steps in one chromedp.Run
	if err := s.driver.LoginWithCookies(url, browserCookies, 20); err != nil {
		return err
	}

	s.logger.Info("Login with cookies succeeded")
	return nil
}

// loginWithUserPassword attempts to login using username and password.
func (s *Session) loginWithUserPassword(url string) error {
	// Use the driver's LoginWithPassword method which executes all steps in one chromedp.Run
	if err := s.driver.LoginWithPassword(url, s.account.UserName, s.account.Password, 20); err != nil {
		return err
	}

	s.logger.Info("Login with user password succeeded")
	return nil
}

// waitLoadingGame waits for the game to fully load by detecting known scenes.
func (s *Session) waitLoadingGame() error {
	const maxAttempts = 10
	const waitInterval = 2 * time.Second

	for i := 0; i < maxAttempts; i++ {
		s.logger.Info("Waiting for game to load", "user", s.account.Identity(), "round", i+1)

		select {
		case <-s.ctx.Done():
			return s.ctx.Err()
		case <-time.After(waitInterval):
		}

		img, err := s.screenCap.Capture(s.ctx)
		if err != nil {
			continue
		}

		// Check for known scenes
		scene := s.sceneRegistry.FindMatch(img, s.sceneMatcher, "user_agreement", "main_city")
		if scene == nil {
			continue
		}

		if scene.Name == "user_agreement" {
			// Click agree button
			if action, ok := scene.Actions["Agree"]; ok {
				if err := s.browserCtrl.Click(s.ctx, action.Point.X, action.Point.Y); err != nil {
					s.logger.Warn("Click agreement failed", "error", err)
					continue
				}
			}
			return nil // Agreement clicked, game will load
		}

		if scene.Name == "main_city" {
			return nil // Game loaded successfully
		}
	}

	return fmt.Errorf("timeout waiting for game to load after %d attempts", maxAttempts)
}

// saveCookiesAfterLogin retrieves and stores cookies after successful login.
func (s *Session) saveCookiesAfterLogin() error {
	cookies, err := s.browserCtrl.GetCookies(s.ctx)
	if err != nil {
		return fmt.Errorf("failed to get cookies: %w", err)
	}

	// Convert to domain cookies
	domainCookies := make([]account.Cookie, len(cookies))
	for i, c := range cookies {
		domainCookies[i] = account.Cookie{
			Name:         c.Name,
			Value:        c.Value,
			Domain:       c.Domain,
			Path:         c.Path,
			HTTPOnly:     c.HTTPOnly,
			Secure:       c.Secure,
			SourcePort:   c.SourcePort,
			SourceScheme: c.SourceScheme,
			Priority:     c.Priority,
		}
	}

	s.account.Cookies = domainCookies
	s.logger.Info("Cookies captured", "count", len(cookies))
	return nil
}
