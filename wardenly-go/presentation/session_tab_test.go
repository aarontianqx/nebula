package presentation

import (
	"image/color"
	"testing"

	"wardenly-go/core/state"
)

func TestSessionTabConfig(t *testing.T) {
	cfg := &SessionTabConfig{
		SessionID:   "session-1",
		AccountName: "Test Account",
	}

	if cfg.SessionID != "session-1" {
		t.Errorf("SessionID = %v, want session-1", cfg.SessionID)
	}
	if cfg.AccountName != "Test Account" {
		t.Errorf("AccountName = %v, want Test Account", cfg.AccountName)
	}
}

func TestColorToString(t *testing.T) {
	tests := []struct {
		name     string
		color    color.Color
		expected string
	}{
		{
			name:     "black",
			color:    color.RGBA{0, 0, 0, 255},
			expected: "RGBA(0, 0, 0, 255)",
		},
		{
			name:     "white",
			color:    color.RGBA{255, 255, 255, 255},
			expected: "RGBA(255, 255, 255, 255)",
		},
		{
			name:     "red",
			color:    color.RGBA{255, 0, 0, 255},
			expected: "RGBA(255, 0, 0, 255)",
		},
		{
			name:     "semi-transparent",
			color:    color.RGBA{128, 128, 128, 128},
			expected: "RGBA(128, 128, 128, 128)",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := colorToString(tt.color)
			if result != tt.expected {
				t.Errorf("colorToString() = %v, want %v", result, tt.expected)
			}
		})
	}
}

func TestSessionState_UIBehavior(t *testing.T) {
	tests := []struct {
		state          state.SessionState
		enableControls bool
	}{
		{state.StateIdle, false},
		{state.StateStarting, false},
		{state.StateLoggingIn, false},
		{state.StateReady, true},
		{state.StateScriptRunning, true},
		{state.StateStopping, false},
		{state.StateStopped, false},
	}

	for _, tt := range tests {
		t.Run(tt.state.String(), func(t *testing.T) {
			canAccept := tt.state.CanAcceptOperations()
			if canAccept != tt.enableControls {
				t.Errorf("State %v: CanAcceptOperations() = %v, want %v",
					tt.state, canAccept, tt.enableControls)
			}
		})
	}
}
