package presentation

import (
	"image"
	"testing"

	"wardenly-go/core/event"
	"wardenly-go/core/state"
)

func TestUICallbacks_Nil(t *testing.T) {
	// Test that nil callbacks don't panic
	callbacks := &UICallbacks{}

	// All callbacks should be nil by default
	if callbacks.OnSessionStarted != nil {
		t.Error("OnSessionStarted should be nil by default")
	}
	if callbacks.OnSessionStopped != nil {
		t.Error("OnSessionStopped should be nil by default")
	}
	if callbacks.OnSessionStateChanged != nil {
		t.Error("OnSessionStateChanged should be nil by default")
	}
}

func TestUICallbacks_Set(t *testing.T) {
	var called bool

	callbacks := &UICallbacks{
		OnSessionStarted: func(sessionID, accountName string) {
			called = true
		},
	}

	callbacks.OnSessionStarted("session-1", "Test Account")

	if !called {
		t.Error("OnSessionStarted callback was not called")
	}
}

func TestUICallbacks_AllCallbacks(t *testing.T) {
	callCount := 0

	callbacks := &UICallbacks{
		OnSessionStarted: func(sessionID, accountName string) {
			callCount++
		},
		OnSessionStopped: func(sessionID string, err error) {
			callCount++
		},
		OnSessionStateChanged: func(sessionID string, oldState, newState state.SessionState) {
			callCount++
		},
		OnScreenCaptured: func(sessionID string, img image.Image) {
			callCount++
		},
		OnLoginSucceeded: func(sessionID string) {
			callCount++
		},
		OnLoginFailed: func(sessionID string, err error) {
			callCount++
		},
		OnCookiesSaved: func(sessionID string) {
			callCount++
		},
		OnOperationFailed: func(sessionID, operation string, err error) {
			callCount++
		},
		OnScriptStarted: func(sessionID, scriptName string) {
			callCount++
		},
		OnScriptStopped: func(sessionID, scriptName string, reason event.StopReason, err error) {
			callCount++
		},
		OnScriptSelectionChanged: func(sessionID, scriptName string) {
			callCount++
		},
	}

	// Call all callbacks
	callbacks.OnSessionStarted("s1", "acc1")
	callbacks.OnSessionStopped("s1", nil)
	callbacks.OnSessionStateChanged("s1", state.StateIdle, state.StateStarting)
	callbacks.OnScreenCaptured("s1", nil)
	callbacks.OnLoginSucceeded("s1")
	callbacks.OnLoginFailed("s1", nil)
	callbacks.OnCookiesSaved("s1")
	callbacks.OnOperationFailed("s1", "click", nil)
	callbacks.OnScriptStarted("s1", "test")
	callbacks.OnScriptStopped("s1", "test", event.StopReasonNormal, nil)
	callbacks.OnScriptSelectionChanged("s1", "test")

	if callCount != 11 {
		t.Errorf("Expected 11 callbacks, got %d", callCount)
	}
}

func TestBridgeConfig(t *testing.T) {
	cfg := &BridgeConfig{}

	if cfg.Coordinator != nil {
		t.Error("Coordinator should be nil by default")
	}
	if cfg.EventBus != nil {
		t.Error("EventBus should be nil by default")
	}
	if cfg.Logger != nil {
		t.Error("Logger should be nil by default")
	}
}
