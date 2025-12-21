// Package state defines the session state machine.
package state

import "fmt"

// SessionState represents the state of a session.
type SessionState int

const (
	// StateIdle is the initial state before the session starts.
	StateIdle SessionState = iota
	// StateStarting indicates the browser is being initialized.
	StateStarting
	// StateLoggingIn indicates the login process is in progress.
	StateLoggingIn
	// StateReady indicates the session is ready to accept operations.
	StateReady
	// StateScriptRunning indicates a script is currently executing.
	StateScriptRunning
	// StateStopping indicates the session is shutting down.
	StateStopping
	// StateStopped indicates the session has been terminated.
	StateStopped
)

// String returns the string representation of the state.
func (s SessionState) String() string {
	switch s {
	case StateIdle:
		return "Idle"
	case StateStarting:
		return "Starting"
	case StateLoggingIn:
		return "LoggingIn"
	case StateReady:
		return "Ready"
	case StateScriptRunning:
		return "ScriptRunning"
	case StateStopping:
		return "Stopping"
	case StateStopped:
		return "Stopped"
	default:
		return fmt.Sprintf("Unknown(%d)", s)
	}
}

// validTransitions defines the allowed state transitions.
// Key is the current state, value is a list of valid target states.
var validTransitions = map[SessionState][]SessionState{
	StateIdle:          {StateStarting},
	StateStarting:      {StateLoggingIn, StateStopping, StateStopped},
	StateLoggingIn:     {StateReady, StateStopping, StateStopped},
	StateReady:         {StateScriptRunning, StateStopping},
	StateScriptRunning: {StateReady, StateStopping},
	StateStopping:      {StateStopped},
	StateStopped:       {}, // Terminal state, no transitions allowed
}

// CanTransitionTo checks if transitioning from the current state to the target state is valid.
func (s SessionState) CanTransitionTo(target SessionState) bool {
	allowed, ok := validTransitions[s]
	if !ok {
		return false
	}
	for _, t := range allowed {
		if t == target {
			return true
		}
	}
	return false
}

// ValidTransitions returns the list of valid target states from the current state.
func (s SessionState) ValidTransitions() []SessionState {
	return validTransitions[s]
}

// IsTerminal returns true if the state is a terminal state (no further transitions).
func (s SessionState) IsTerminal() bool {
	return s == StateStopped
}

// IsActive returns true if the session is in an active state (not idle or stopped).
func (s SessionState) IsActive() bool {
	return s != StateIdle && s != StateStopped
}

// CanAcceptOperations returns true if the session can accept user operations.
// This includes LoggingIn state to allow screen capture during login.
func (s SessionState) CanAcceptOperations() bool {
	return s == StateLoggingIn || s == StateReady || s == StateScriptRunning
}

// CanStartScript returns true if a script can be started in this state.
func (s SessionState) CanStartScript() bool {
	return s == StateReady
}

// CanStopScript returns true if a script can be stopped in this state.
func (s SessionState) CanStopScript() bool {
	return s == StateScriptRunning
}

// TransitionError represents an invalid state transition attempt.
type TransitionError struct {
	From   SessionState
	To     SessionState
	Reason string
}

func (e *TransitionError) Error() string {
	if e.Reason != "" {
		return fmt.Sprintf("invalid state transition from %s to %s: %s", e.From, e.To, e.Reason)
	}
	return fmt.Sprintf("invalid state transition from %s to %s", e.From, e.To)
}

// NewTransitionError creates a new TransitionError.
func NewTransitionError(from, to SessionState, reason string) *TransitionError {
	return &TransitionError{From: from, To: to, Reason: reason}
}
