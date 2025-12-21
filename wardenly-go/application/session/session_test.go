package session

import (
	"testing"

	"wardenly-go/core/state"
	"wardenly-go/domain/account"
)

func TestConfig_Defaults(t *testing.T) {
	acc := &account.Account{
		ID:       "test-id",
		RoleName: "TestRole",
	}

	cfg := &Config{
		ID:      "session-1",
		Account: acc,
	}

	if cfg.ID != "session-1" {
		t.Errorf("ID = %v, want session-1", cfg.ID)
	}
	if cfg.Account != acc {
		t.Error("Account not set correctly")
	}
}

func TestSession_Identity(t *testing.T) {
	acc := &account.Account{
		ID:       "test-id",
		RoleName: "TestRole",
	}

	// Note: We can't fully test Session without mocking dependencies,
	// but we can test the config structure
	cfg := &Config{
		ID:            "session-1",
		Account:       acc,
		CommandBuffer: 50,
	}

	if cfg.CommandBuffer != 50 {
		t.Errorf("CommandBuffer = %d, want 50", cfg.CommandBuffer)
	}
}

func TestStateTransitions(t *testing.T) {
	// Test state transition logic used by Session
	tests := []struct {
		name     string
		from     state.SessionState
		to       state.SessionState
		expected bool
	}{
		{"Idle to Starting", state.StateIdle, state.StateStarting, true},
		{"Starting to LoggingIn", state.StateStarting, state.StateLoggingIn, true},
		{"LoggingIn to Ready", state.StateLoggingIn, state.StateReady, true},
		{"Ready to ScriptRunning", state.StateReady, state.StateScriptRunning, true},
		{"ScriptRunning to Ready", state.StateScriptRunning, state.StateReady, true},
		{"Ready to Stopping", state.StateReady, state.StateStopping, true},
		{"Stopping to Stopped", state.StateStopping, state.StateStopped, true},
		// Invalid transitions
		{"Idle to Ready (invalid)", state.StateIdle, state.StateReady, false},
		{"Ready to Idle (invalid)", state.StateReady, state.StateIdle, false},
		{"Stopped to Starting (invalid)", state.StateStopped, state.StateStarting, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.from.CanTransitionTo(tt.to); got != tt.expected {
				t.Errorf("CanTransitionTo() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestStateHelpers(t *testing.T) {
	tests := []struct {
		state          state.SessionState
		canAcceptOps   bool
		canStartScript bool
		canStopScript  bool
	}{
		{state.StateIdle, false, false, false},
		{state.StateStarting, false, false, false},
		{state.StateLoggingIn, false, false, false},
		{state.StateReady, true, true, false},
		{state.StateScriptRunning, true, false, true},
		{state.StateStopping, false, false, false},
		{state.StateStopped, false, false, false},
	}

	for _, tt := range tests {
		t.Run(tt.state.String(), func(t *testing.T) {
			if got := tt.state.CanAcceptOperations(); got != tt.canAcceptOps {
				t.Errorf("CanAcceptOperations() = %v, want %v", got, tt.canAcceptOps)
			}
			if got := tt.state.CanStartScript(); got != tt.canStartScript {
				t.Errorf("CanStartScript() = %v, want %v", got, tt.canStartScript)
			}
			if got := tt.state.CanStopScript(); got != tt.canStopScript {
				t.Errorf("CanStopScript() = %v, want %v", got, tt.canStopScript)
			}
		})
	}
}
