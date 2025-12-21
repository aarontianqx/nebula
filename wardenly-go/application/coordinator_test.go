package application

import (
	"testing"

	"wardenly-go/core/eventbus"
	domainscene "wardenly-go/domain/scene"
	domainscript "wardenly-go/domain/script"
	"wardenly-go/infrastructure/browser"
	"wardenly-go/infrastructure/ocr"
)

func TestCoordinatorConfig(t *testing.T) {
	eventBus := eventbus.New(10)
	defer eventBus.Close()

	sceneReg := domainscene.NewRegistry()
	scriptReg := domainscript.NewRegistry()
	ocrClient := ocr.NewNoOpClient()

	cfg := &CoordinatorConfig{
		EventBus:       eventBus,
		SceneRegistry:  sceneReg,
		ScriptRegistry: scriptReg,
		OCRClient:      ocrClient,
		DriverFactory: func() browser.Driver {
			return browser.NewChromeDPDriver(nil)
		},
	}

	if cfg.EventBus == nil {
		t.Error("EventBus not set")
	}
	if cfg.SceneRegistry == nil {
		t.Error("SceneRegistry not set")
	}
	if cfg.ScriptRegistry == nil {
		t.Error("ScriptRegistry not set")
	}
	if cfg.OCRClient == nil {
		t.Error("OCRClient not set")
	}
	if cfg.DriverFactory == nil {
		t.Error("DriverFactory not set")
	}
}

func TestNewCoordinator(t *testing.T) {
	eventBus := eventbus.New(10)
	defer eventBus.Close()

	cfg := &CoordinatorConfig{
		EventBus:       eventBus,
		SceneRegistry:  domainscene.NewRegistry(),
		ScriptRegistry: domainscript.NewRegistry(),
	}

	coord := NewCoordinator(cfg)
	if coord == nil {
		t.Fatal("NewCoordinator returned nil")
	}

	if coord.sessions == nil {
		t.Error("sessions map not initialized")
	}
	if coord.eventBus != eventBus {
		t.Error("eventBus not set correctly")
	}
}

func TestCoordinator_SessionCount(t *testing.T) {
	cfg := &CoordinatorConfig{
		SceneRegistry:  domainscene.NewRegistry(),
		ScriptRegistry: domainscript.NewRegistry(),
	}

	coord := NewCoordinator(cfg)
	defer coord.Stop()

	if coord.SessionCount() != 0 {
		t.Errorf("SessionCount() = %d, want 0", coord.SessionCount())
	}
}

func TestCoordinator_GetSession_NotFound(t *testing.T) {
	cfg := &CoordinatorConfig{
		SceneRegistry:  domainscene.NewRegistry(),
		ScriptRegistry: domainscript.NewRegistry(),
	}

	coord := NewCoordinator(cfg)
	defer coord.Stop()

	sess := coord.GetSession("nonexistent")
	if sess != nil {
		t.Error("Expected nil for nonexistent session")
	}
}

func TestCoordinator_GetAllSessions_Empty(t *testing.T) {
	cfg := &CoordinatorConfig{
		SceneRegistry:  domainscene.NewRegistry(),
		ScriptRegistry: domainscript.NewRegistry(),
	}

	coord := NewCoordinator(cfg)
	defer coord.Stop()

	sessions := coord.GetAllSessions()
	if len(sessions) != 0 {
		t.Errorf("GetAllSessions() returned %d sessions, want 0", len(sessions))
	}
}

func TestCoordinator_GetActiveSessions_Empty(t *testing.T) {
	cfg := &CoordinatorConfig{
		SceneRegistry:  domainscene.NewRegistry(),
		ScriptRegistry: domainscript.NewRegistry(),
	}

	coord := NewCoordinator(cfg)
	defer coord.Stop()

	sessions := coord.GetActiveSessions()
	if len(sessions) != 0 {
		t.Errorf("GetActiveSessions() returned %d sessions, want 0", len(sessions))
	}
}

func TestCoordinator_StartStop(t *testing.T) {
	cfg := &CoordinatorConfig{
		SceneRegistry:  domainscene.NewRegistry(),
		ScriptRegistry: domainscript.NewRegistry(),
	}

	coord := NewCoordinator(cfg)
	coord.Start()

	// Should not panic
	coord.Stop()
}
