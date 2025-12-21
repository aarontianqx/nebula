package state

import "testing"

func TestSessionState_String(t *testing.T) {
	tests := []struct {
		state    SessionState
		expected string
	}{
		{StateIdle, "Idle"},
		{StateStarting, "Starting"},
		{StateLoggingIn, "LoggingIn"},
		{StateReady, "Ready"},
		{StateScriptRunning, "ScriptRunning"},
		{StateStopping, "Stopping"},
		{StateStopped, "Stopped"},
		{SessionState(99), "Unknown(99)"},
	}

	for _, tt := range tests {
		t.Run(tt.expected, func(t *testing.T) {
			if got := tt.state.String(); got != tt.expected {
				t.Errorf("SessionState.String() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestSessionState_CanTransitionTo(t *testing.T) {
	tests := []struct {
		name     string
		from     SessionState
		to       SessionState
		expected bool
	}{
		// Valid transitions from Idle
		{"Idle -> Starting", StateIdle, StateStarting, true},
		{"Idle -> Ready (invalid)", StateIdle, StateReady, false},

		// Valid transitions from Starting
		{"Starting -> LoggingIn", StateStarting, StateLoggingIn, true},
		{"Starting -> Stopping", StateStarting, StateStopping, true},
		{"Starting -> Stopped", StateStarting, StateStopped, true},
		{"Starting -> Ready (invalid)", StateStarting, StateReady, false},

		// Valid transitions from LoggingIn
		{"LoggingIn -> Ready", StateLoggingIn, StateReady, true},
		{"LoggingIn -> Stopping", StateLoggingIn, StateStopping, true},
		{"LoggingIn -> Stopped", StateLoggingIn, StateStopped, true},

		// Valid transitions from Ready
		{"Ready -> ScriptRunning", StateReady, StateScriptRunning, true},
		{"Ready -> Stopping", StateReady, StateStopping, true},
		{"Ready -> Idle (invalid)", StateReady, StateIdle, false},

		// Valid transitions from ScriptRunning
		{"ScriptRunning -> Ready", StateScriptRunning, StateReady, true},
		{"ScriptRunning -> Stopping", StateScriptRunning, StateStopping, true},
		{"ScriptRunning -> Idle (invalid)", StateScriptRunning, StateIdle, false},

		// Valid transitions from Stopping
		{"Stopping -> Stopped", StateStopping, StateStopped, true},
		{"Stopping -> Ready (invalid)", StateStopping, StateReady, false},

		// Stopped is terminal
		{"Stopped -> Idle (invalid)", StateStopped, StateIdle, false},
		{"Stopped -> Starting (invalid)", StateStopped, StateStarting, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.from.CanTransitionTo(tt.to); got != tt.expected {
				t.Errorf("CanTransitionTo() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestSessionState_IsTerminal(t *testing.T) {
	tests := []struct {
		state    SessionState
		expected bool
	}{
		{StateIdle, false},
		{StateStarting, false},
		{StateLoggingIn, false},
		{StateReady, false},
		{StateScriptRunning, false},
		{StateStopping, false},
		{StateStopped, true},
	}

	for _, tt := range tests {
		t.Run(tt.state.String(), func(t *testing.T) {
			if got := tt.state.IsTerminal(); got != tt.expected {
				t.Errorf("IsTerminal() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestSessionState_IsActive(t *testing.T) {
	tests := []struct {
		state    SessionState
		expected bool
	}{
		{StateIdle, false},
		{StateStarting, true},
		{StateLoggingIn, true},
		{StateReady, true},
		{StateScriptRunning, true},
		{StateStopping, true},
		{StateStopped, false},
	}

	for _, tt := range tests {
		t.Run(tt.state.String(), func(t *testing.T) {
			if got := tt.state.IsActive(); got != tt.expected {
				t.Errorf("IsActive() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestSessionState_CanAcceptOperations(t *testing.T) {
	tests := []struct {
		state    SessionState
		expected bool
	}{
		{StateIdle, false},
		{StateStarting, false},
		{StateLoggingIn, false},
		{StateReady, true},
		{StateScriptRunning, true},
		{StateStopping, false},
		{StateStopped, false},
	}

	for _, tt := range tests {
		t.Run(tt.state.String(), func(t *testing.T) {
			if got := tt.state.CanAcceptOperations(); got != tt.expected {
				t.Errorf("CanAcceptOperations() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestSessionState_CanStartScript(t *testing.T) {
	tests := []struct {
		state    SessionState
		expected bool
	}{
		{StateIdle, false},
		{StateStarting, false},
		{StateLoggingIn, false},
		{StateReady, true},
		{StateScriptRunning, false},
		{StateStopping, false},
		{StateStopped, false},
	}

	for _, tt := range tests {
		t.Run(tt.state.String(), func(t *testing.T) {
			if got := tt.state.CanStartScript(); got != tt.expected {
				t.Errorf("CanStartScript() = %v, want %v", got, tt.expected)
			}
		})
	}
}

func TestTransitionError_Error(t *testing.T) {
	tests := []struct {
		name     string
		err      *TransitionError
		expected string
	}{
		{
			"with reason",
			NewTransitionError(StateIdle, StateReady, "not allowed"),
			"invalid state transition from Idle to Ready: not allowed",
		},
		{
			"without reason",
			NewTransitionError(StateIdle, StateReady, ""),
			"invalid state transition from Idle to Ready",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.err.Error(); got != tt.expected {
				t.Errorf("Error() = %v, want %v", got, tt.expected)
			}
		})
	}
}
